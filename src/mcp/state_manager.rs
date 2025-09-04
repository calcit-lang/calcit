//! Global state manager for Calcit MCP server
//!
//! This module manages the global state including:
//! - Current module state (readable and writable)
//! - Dependency modules cache (HashMap<String, Snapshot> where key is namespace)

use crate::snapshot::Snapshot;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Global state manager for the MCP server
#[derive(Clone)]
pub struct StateManager {
  /// Current module snapshot (readable and writable)
  current_module: Arc<RwLock<Option<Snapshot>>>,
  /// Dependency modules cache, key is namespace (root namespace of the package)
  dependency_cache: Arc<RwLock<HashMap<String, Snapshot>>>,
  /// Path to the current module's compact.cirru file
  compact_cirru_path: String,
}

impl StateManager {
  /// Create a new state manager
  pub fn new(compact_cirru_path: String) -> Self {
    Self {
      current_module: Arc::new(RwLock::new(None)),
      dependency_cache: Arc::new(RwLock::new(HashMap::new())),
      compact_cirru_path,
    }
  }

  /// Load the current module from file if not already loaded
  pub fn ensure_current_module_loaded(&self) -> Result<(), String> {
    let current_guard = self
      .current_module
      .read()
      .map_err(|e| format!("Failed to read current module lock: {e}"))?;
    if current_guard.is_some() {
      return Ok(());
    }
    drop(current_guard);

    // Load from file
    let snapshot = self.load_snapshot_from_file(&self.compact_cirru_path)?;

    let mut current_guard = self
      .current_module
      .write()
      .map_err(|e| format!("Failed to write current module lock: {e}"))?;
    *current_guard = Some(snapshot);

    Ok(())
  }

  /// Get a read-only reference to the current module
  pub fn get_current_module(&self) -> Result<Arc<RwLock<Option<Snapshot>>>, String> {
    self.ensure_current_module_loaded()?;
    Ok(self.current_module.clone())
  }

  /// Update the current module and save to file
  pub fn update_current_module<F>(&self, update_fn: F) -> Result<(), String>
  where
    F: FnOnce(&mut Snapshot) -> Result<(), String>,
  {
    self.ensure_current_module_loaded()?;

    let mut current_guard = self
      .current_module
      .write()
      .map_err(|e| format!("Failed to write current module lock: {e}"))?;

    if let Some(ref mut snapshot) = *current_guard {
      update_fn(snapshot)?;

      // Save to file
      self.save_snapshot_to_file(&self.compact_cirru_path, snapshot)?;
    } else {
      return Err("Current module not loaded".to_string());
    }

    Ok(())
  }

  /// Get a dependency module by namespace, load from file if not cached
  pub fn get_dependency_module(&self, namespace: &str) -> Result<Snapshot, String> {
    // First check cache
    {
      let cache_guard = self
        .dependency_cache
        .read()
        .map_err(|e| format!("Failed to read dependency cache lock: {e}"))?;
      if let Some(snapshot) = cache_guard.get(namespace) {
        return Ok(snapshot.clone());
      }
    }

    // Load from file
    let snapshot = self.load_dependency_from_file(namespace)?;

    // Cache it
    {
      let mut cache_guard = self
        .dependency_cache
        .write()
        .map_err(|e| format!("Failed to write dependency cache lock: {e}"))?;
      cache_guard.insert(namespace.to_string(), snapshot.clone());
    }

    Ok(snapshot)
  }

  /// Clear the dependency cache (useful for development/testing)
  pub fn clear_dependency_cache(&self) -> Result<(), String> {
    let mut cache_guard = self
      .dependency_cache
      .write()
      .map_err(|e| format!("Failed to write dependency cache lock: {e}"))?;
    cache_guard.clear();
    Ok(())
  }

  /// Reload the current module from file
  pub fn reload_current_module(&self) -> Result<(), String> {
    let snapshot = self.load_snapshot_from_file(&self.compact_cirru_path)?;

    let mut current_guard = self
      .current_module
      .write()
      .map_err(|e| format!("Failed to write current module lock: {e}"))?;
    *current_guard = Some(snapshot);

    Ok(())
  }

  /// Get the current module's snapshot (for read-only access)
  pub fn with_current_module<F, R>(&self, f: F) -> Result<R, String>
  where
    F: FnOnce(&Snapshot) -> R,
  {
    self.ensure_current_module_loaded()?;

    let current_guard = self
      .current_module
      .read()
      .map_err(|e| format!("Failed to read current module lock: {e}"))?;

    if let Some(ref snapshot) = *current_guard {
      Ok(f(snapshot))
    } else {
      Err("Current module not loaded".to_string())
    }
  }

  /// Load snapshot from file
  fn load_snapshot_from_file(&self, path: &str) -> Result<Snapshot, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("Failed to read compact.cirru: {e}"))?;

    let edn_data = cirru_edn::parse(&content).map_err(|e| format!("Failed to parse compact.cirru as EDN: {e}"))?;

    crate::snapshot::load_snapshot_data(&edn_data, path).map_err(|e| format!("Failed to load snapshot: {e}"))
  }

  /// Load dependency module from file
  fn load_dependency_from_file(&self, namespace: &str) -> Result<Snapshot, String> {
    // For now, we'll use the same logic as the original load_snapshot
    // In the future, this could be enhanced to load from module directories
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let module_folder = Path::new(&home_dir).join(".config/calcit/modules");

    // Try to find the module file
    let module_path = module_folder.join(format!("{namespace}/compact.cirru"));

    if module_path.exists() {
      self.load_snapshot_from_file(&module_path.display().to_string())
    } else {
      Err(format!("Dependency module not found: {namespace}"))
    }
  }

  /// Save snapshot to file
  fn save_snapshot_to_file(&self, path: &str, snapshot: &Snapshot) -> Result<(), String> {
    super::cirru_utils::save_snapshot_to_file(path, snapshot)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_state_manager_creation() {
    let manager = StateManager::new("/tmp/test.cirru".to_string());
    assert_eq!(manager.compact_cirru_path, "/tmp/test.cirru");
  }

  #[test]
  fn test_file_save_functionality() {
    use crate::snapshot::{Snapshot, SnapshotConfigs};
    use std::collections::HashMap;
    use std::fs;

    let test_file = "/tmp/test_state_manager.cirru";
    let state_manager = StateManager::new(test_file.to_string());

    // Create a test snapshot
    let snapshot = Snapshot {
      package: "test-package".to_string(),
      configs: SnapshotConfigs {
        init_fn: "test.init".to_string(),
        reload_fn: "test.reload".to_string(),
        version: "0.1.0".to_string(),
        modules: vec!["test".to_string()],
      },
      entries: HashMap::new(),
      files: HashMap::new(),
      docs: None,
    };

    // Manually set the current module
    {
      let mut current_guard = state_manager.current_module.write().unwrap();
      *current_guard = Some(snapshot.clone());
    }

    // Test update_current_module saves to file
    let result = state_manager.update_current_module(|snapshot| {
      snapshot.package = "updated-package".to_string();
      Ok(())
    });

    assert!(result.is_ok(), "update_current_module should succeed");

    // Verify file was created
    assert!(fs::metadata(test_file).is_ok(), "File should be created");

    // Clean up
    let _ = fs::remove_file(test_file);
  }

  #[test]
  fn test_dependency_cache() {
    let manager = StateManager::new("/tmp/test.cirru".to_string());

    // Clear cache should work even when empty
    assert!(manager.clear_dependency_cache().is_ok());
  }
}
