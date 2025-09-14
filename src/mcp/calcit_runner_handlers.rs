//! Calcit Runner Management Handlers
//!
//! This module provides handlers for managing Calcit runner processes in background mode.
//! It includes functionality for:
//! - Starting `cr <filename>` commands in background
//! - Collecting and managing logs in a queue
//! - Stopping processes and retrieving remaining logs

use super::tools::{GenerateCalcitIncrementalRequest, GrabCalcitRunnerLogsRequest, StartCalcitRunnerRequest, StopCalcitRunnerRequest};
use crate::snapshot::{ChangesDict, FileChangeInfo, FileInSnapShot};
use axum::response::Json as ResponseJson;
use cirru_edn;
use serde_json::Value;
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime};

/// Log entry with timestamp
#[derive(Debug, Clone)]
pub struct LogEntry {
  pub timestamp: SystemTime,
  pub content: String,
  pub is_error: bool,
}

/// Runner state
#[derive(Debug)]
pub enum RunnerState {
  Stopped,
  Starting,
  Running(Child),
  Failed(String),
}

/// Global runner manager
#[derive(Debug)]
pub struct CalcitRunnerManager {
  pub state: RunnerState,
  pub logs: VecDeque<LogEntry>,
  pub max_logs: usize,
}

impl Default for CalcitRunnerManager {
  fn default() -> Self {
    Self::new()
  }
}

impl CalcitRunnerManager {
  pub fn new() -> Self {
    Self {
      state: RunnerState::Stopped,
      logs: VecDeque::new(),
      max_logs: 1000, // Limit log queue size
    }
  }

  pub fn add_log(&mut self, content: String, is_error: bool) {
    // Print directly to stdout for debugging
    if is_error {
      eprintln!("\x1b[31m✗ {content}\x1b[0m");
    } else {
      println!("\x1b[34m◆ {content}\x1b[0m");
    }

    let entry = LogEntry {
      timestamp: SystemTime::now(),
      content,
      is_error,
    };

    self.logs.push_back(entry);

    // Keep only the most recent logs
    while self.logs.len() > self.max_logs {
      self.logs.pop_front();
    }
  }

  pub fn grab_logs(&mut self) -> Vec<LogEntry> {
    self.logs.drain(..).collect()
  }

  pub fn is_running(&self) -> bool {
    matches!(self.state, RunnerState::Running(_))
  }

  pub fn get_status(&self) -> String {
    match &self.state {
      RunnerState::Stopped => "stopped".to_string(),
      RunnerState::Starting => "starting".to_string(),
      RunnerState::Running(_) => "running".to_string(),
      RunnerState::Failed(err) => format!("failed: {err}"),
    }
  }
}

// Global runner manager instance
static RUNNER_MANAGER: OnceLock<Arc<Mutex<CalcitRunnerManager>>> = OnceLock::new();

fn get_runner_manager() -> Arc<Mutex<CalcitRunnerManager>> {
  RUNNER_MANAGER
    .get_or_init(|| Arc::new(Mutex::new(CalcitRunnerManager::new())))
    .clone()
}

/// Start a Calcit runner in background mode
pub fn start_calcit_runner(_app_state: &super::AppState, request: StartCalcitRunnerRequest) -> ResponseJson<Value> {
  let manager = get_runner_manager();
  let mut manager_guard = match manager.lock() {
    Ok(guard) => guard,
    Err(e) => {
      return ResponseJson(serde_json::json!({
          "error": format!("Failed to acquire runner manager lock: {}", e)
      }));
    }
  };

  // Check if already running
  if manager_guard.is_running() {
    return ResponseJson(serde_json::json!({
        "error": "Calcit runner is already running. Stop it first before starting a new one."
    }));
  }

  // Create .calcit-runner.cirru file in the same directory as the source file
  let source_file = Path::new(&request.filename);
  let source_dir = source_file.parent().unwrap_or(Path::new("."));
  let runner_file = source_dir.join(".calcit-runner.cirru");

  if source_file.exists() {
    if let Err(e) = fs::copy(source_file, &runner_file) {
      return ResponseJson(serde_json::json!({
          "error": format!("Failed to copy {} to .calcit-runner.cirru: {}", request.filename, e)
      }));
    }
  } else {
    return ResponseJson(serde_json::json!({
        "error": format!("Source file {} not found", request.filename)
    }));
  }

  // Clear old logs
  manager_guard.logs.clear();
  manager_guard.state = RunnerState::Starting;

  let filename = request.filename.clone();
  let mode = &request.mode;

  // Validate mode
  if let Err(err) = super::tools::validate_calcit_runner_mode(mode) {
    manager_guard.state = RunnerState::Failed(err.clone());
    manager_guard.add_log(format!("Invalid mode: {err}"), true);
    return ResponseJson(serde_json::json!({
      "success": false,
      "error": err
    }));
  }

  manager_guard.add_log(format!("Starting Calcit runner with file: {filename}, mode: {mode}"), false);

  // Start the cr command with appropriate mode
  let mut command = Command::new("cr");
  command
    .arg(runner_file.to_string_lossy().as_ref())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

  // Add mode-specific arguments
  match mode.trim().to_lowercase().as_str() {
    "" | "run" => {
      // Default mode, no additional arguments needed
    }
    "js" => {
      command.arg("--emit-js");
    }
    _ => {
      // This should not happen due to validation above, but handle it just in case
      let err = format!("Unexpected mode: {mode}");
      manager_guard.state = RunnerState::Failed(err.clone());
      manager_guard.add_log(err.clone(), true);
      return ResponseJson(serde_json::json!({
        "success": false,
        "error": err
      }));
    }
  }

  match command.spawn() {
    Ok(mut child) => {
      // Take stdout and stderr for monitoring
      let stdout = child.stdout.take();
      let stderr = child.stderr.take();

      manager_guard.state = RunnerState::Running(child);
      manager_guard.add_log("Calcit runner started successfully".to_string(), false);

      // Clone manager for background threads
      let manager_clone = manager.clone();
      let manager_clone2 = manager.clone();

      // Spawn thread to monitor stdout
      if let Some(stdout) = stdout {
        thread::spawn(move || {
          let reader = BufReader::new(stdout);
          for line in reader.lines() {
            match line {
              Ok(content) => {
                if let Ok(mut guard) = manager_clone.lock() {
                  guard.add_log(content, false);
                }
              }
              Err(_) => break,
            }
          }
        });
      }

      // Spawn thread to monitor stderr
      if let Some(stderr) = stderr {
        thread::spawn(move || {
          let reader = BufReader::new(stderr);
          for line in reader.lines() {
            match line {
              Ok(content) => {
                if let Ok(mut guard) = manager_clone2.lock() {
                  guard.add_log(content, true);
                }
              }
              Err(_) => break,
            }
          }
        });
      }

      drop(manager_guard); // Release the lock

      ResponseJson(serde_json::json!({
          "success": true,
          "message": format!("Calcit runner started with file: {}", filename),
          "status": "running"
      }))
    }
    Err(e) => {
      let error_msg = format!("Failed to start Calcit runner: {e}");
      manager_guard.state = RunnerState::Failed(error_msg.clone());
      manager_guard.add_log(error_msg.clone(), true);

      ResponseJson(serde_json::json!({
          "error": error_msg
      }))
    }
  }
}

/// Grab logs from the running Calcit runner
pub fn grab_calcit_runner_logs(_app_state: &super::AppState, _request: GrabCalcitRunnerLogsRequest) -> ResponseJson<Value> {
  let manager = get_runner_manager();
  let mut manager_guard = match manager.lock() {
    Ok(guard) => guard,
    Err(e) => {
      return ResponseJson(serde_json::json!({
          "error": format!("Failed to acquire runner manager lock: {}", e)
      }));
    }
  };

  let logs = manager_guard.grab_logs();
  let status = manager_guard.get_status();

  // Convert logs to JSON format
  let log_entries: Vec<serde_json::Value> = logs
    .into_iter()
    .map(|entry| {
      let timestamp = entry
        .timestamp
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs();

      serde_json::json!({
          "timestamp": timestamp,
          "content": entry.content,
          "is_error": entry.is_error
      })
    })
    .collect();

  ResponseJson(serde_json::json!({
      "success": true,
      "status": status,
      "logs": log_entries,
      "log_count": log_entries.len()
  }))
}

/// Stop the running Calcit runner and retrieve remaining logs
pub fn stop_calcit_runner(_app_state: &super::AppState, _request: StopCalcitRunnerRequest) -> ResponseJson<Value> {
  let manager = get_runner_manager();
  let mut manager_guard = match manager.lock() {
    Ok(guard) => guard,
    Err(e) => {
      return ResponseJson(serde_json::json!({
          "error": format!("Failed to acquire runner manager lock: {}", e)
      }));
    }
  };

  let stop_message = match std::mem::replace(&mut manager_guard.state, RunnerState::Stopped) {
    RunnerState::Running(mut child) => {
      manager_guard.add_log("Stopping Calcit runner...".to_string(), false);

      // Try to terminate the process gracefully
      let message = match child.kill() {
        Ok(_) => {
          let msg = "Calcit runner stopped successfully".to_string();
          manager_guard.add_log(msg.clone(), false);
          msg
        }
        Err(e) => {
          let msg = format!("Error stopping Calcit runner: {e}");
          manager_guard.add_log(msg.clone(), true);
          msg
        }
      };

      // Wait for process to exit
      let _ = child.wait();
      message
    }
    RunnerState::Stopped => "Calcit runner was already stopped".to_string(),
    RunnerState::Starting => "Calcit runner was starting, now stopped".to_string(),
    RunnerState::Failed(err) => {
      format!("Calcit runner was in failed state: {err}")
    }
  };

  // Grab all remaining logs
  let logs = manager_guard.grab_logs();

  // Convert logs to JSON format
  let result_logs: Vec<serde_json::Value> = logs
    .into_iter()
    .map(|entry| {
      let timestamp = entry
        .timestamp
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs();

      serde_json::json!({
          "timestamp": timestamp,
          "content": entry.content,
          "is_error": entry.is_error
      })
    })
    .collect();

  ResponseJson(serde_json::json!({
      "success": true,
      "message": stop_message,
      "status": "stopped",
      "logs": result_logs,
      "log_count": result_logs.len()
  }))
}

/// 生成增量文件
pub fn generate_incremental_file(_app_state: &super::AppState, request: GenerateCalcitIncrementalRequest) -> ResponseJson<Value> {
  let source_file = request.source_file.as_deref().unwrap_or("compact.cirru");
  let current_compact = Path::new(source_file);
  let source_dir = current_compact.parent().unwrap_or(Path::new("."));
  let runner_file = source_dir.join(".calcit-runner.cirru");
  let inc_file = Path::new(".compact-inc.cirru");

  // 检查文件是否存在
  if !current_compact.exists() {
    return ResponseJson(serde_json::json!({
      "error": format!("{} file not found", source_file)
    }));
  }

  if !runner_file.exists() {
    return ResponseJson(serde_json::json!({
      "error": ".calcit-runner.cirru file not found. Please run start_calcit_runner first."
    }));
  }

  // 读取并解析文件
  let current_content = match fs::read_to_string(current_compact) {
    Ok(content) => content,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to read compact.cirru: {}", e)
      }));
    }
  };
  let tmp_content = match fs::read_to_string(&runner_file) {
    Ok(content) => content,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to read .calcit-runner.cirru: {}", e)
      }));
    }
  };

  let current_edn = match cirru_edn::parse(&current_content) {
    Ok(edn) => edn,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to parse current compact.cirru: {}", e)
      }));
    }
  };
  let tmp_edn = match cirru_edn::parse(&tmp_content) {
    Ok(edn) => edn,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to parse tmp compact.cirru: {}", e)
      }));
    }
  };

  // 计算差异
  let changes = match find_compact_changes(&current_edn, &tmp_edn) {
    Ok(changes) => changes,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to calculate changes: {}", e)
      }));
    }
  };

  if changes.is_empty() {
    return ResponseJson(serde_json::json!({
      "success": true,
      "message": "No changes detected",
      "has_changes": false
    }));
  }

  // 生成增量文件
  let changes_edn: cirru_edn::Edn = match changes.try_into() {
    Ok(edn) => edn,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to convert changes to EDN: {}", e)
      }));
    }
  };
  let inc_content = match cirru_edn::format(&changes_edn, true) {
    Ok(content) => content,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to format incremental data: {}", e)
      }));
    }
  };

  if let Err(e) = fs::write(inc_file, inc_content) {
    return ResponseJson(serde_json::json!({
      "error": format!("Failed to write .compact-inc.cirru: {}", e)
    }));
  }

  ResponseJson(serde_json::json!({
    "success": true,
    "message": "Incremental file generated successfully",
    "has_changes": true,
    "inc_file": ".compact-inc.cirru"
  }))
}

/// 从 bundle_calcit.rs 复制的函数，用于计算文件差异
fn find_compact_changes(new_data: &cirru_edn::Edn, old_data: &cirru_edn::Edn) -> Result<ChangesDict, String> {
  use std::collections::{HashMap, HashSet};
  use std::sync::Arc;

  let old_files: HashMap<Arc<str>, FileInSnapShot> = old_data
    .view_map()
    .map_err(|e| format!("Failed to parse old data: {e}"))?
    .get_or_nil("files")
    .try_into()
    .map_err(|e| format!("Failed to parse old files: {e}"))?;

  let new_files: HashMap<Arc<str>, FileInSnapShot> = new_data
    .view_map()
    .map_err(|e| format!("Failed to parse new data: {e}"))?
    .get_or_nil("files")
    .try_into()
    .map_err(|e| format!("Failed to parse new files: {e}"))?;

  let old_namespaces = old_files.keys().collect::<HashSet<_>>();
  let new_namespaces = new_files.keys().collect::<HashSet<_>>();
  let added_namespaces = new_namespaces.difference(&old_namespaces).collect::<HashSet<_>>();
  let common_namespaces = new_namespaces.intersection(&old_namespaces).collect::<HashSet<_>>();
  let removed_namespaces = old_namespaces
    .difference(&new_namespaces)
    .map(|x| (*x).to_owned())
    .collect::<HashSet<Arc<_>>>();
  let added_files = added_namespaces
    .iter()
    .map(|name| ((**name).to_owned(), new_files[**name].to_owned()))
    .collect::<HashMap<Arc<str>, FileInSnapShot>>();

  let mut changed_files: HashMap<Arc<str>, FileChangeInfo> = HashMap::new();
  for namespace in common_namespaces {
    let old_file = old_files[*namespace].to_owned();
    let new_file = new_files[*namespace].to_owned();
    if old_file == new_file {
      continue;
    }
    let changes = find_file_changes(&old_file, &new_file)?;
    changed_files.insert(namespace.to_owned().to_owned(), changes);
  }

  Ok(ChangesDict {
    added: added_files,
    removed: removed_namespaces,
    changed: changed_files,
  })
}

fn find_file_changes(old_file: &FileInSnapShot, new_file: &FileInSnapShot) -> Result<FileChangeInfo, String> {
  use std::collections::{HashMap, HashSet};

  let old_defs = old_file.defs.keys().cloned().collect::<HashSet<String>>();
  let new_defs = new_file.defs.keys().cloned().collect::<HashSet<String>>();

  let added_defs = new_defs
    .difference(&old_defs)
    .map(|name| ((*name).to_owned(), new_file.defs[&**name].code.to_owned()))
    .collect::<HashMap<String, cirru_parser::Cirru>>();

  let removed_defs = old_defs
    .difference(&new_defs)
    .map(|name| (*name).to_owned())
    .collect::<HashSet<String>>();

  let mut changed_defs: HashMap<String, cirru_parser::Cirru> = HashMap::new();
  let common_defs = new_defs.intersection(&old_defs).collect::<HashSet<_>>();
  for def_name in common_defs {
    let old_def = old_file.defs[&**def_name].to_owned();
    let new_def = new_file.defs[&**def_name].to_owned();
    if old_def == new_def {
      continue;
    }
    changed_defs.insert(def_name.to_owned().to_owned(), new_def.code.to_owned());
  }

  Ok(FileChangeInfo {
    ns: if old_file.ns == new_file.ns {
      None
    } else {
      Some(new_file.ns.code.to_owned())
    },
    added_defs,
    removed_defs,
    changed_defs,
  })
}
