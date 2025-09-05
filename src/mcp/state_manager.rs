//! Global state manager for Calcit MCP server
//!
//! This module manages the global state including:
//! - Current module state (readable and writable)
//! - Dependency modules cache (HashMap<String, ModuleWithDoc> where key is namespace)

use crate::snapshot::Snapshot;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Module with documentation data
#[derive(Debug, Clone)]
pub struct DepModuleWithDoc {
  /// package name, aka root namespace of that module
  pub package: String,
  /// module folder name (may differ from package namespace)
  pub module_folder: String,
  pub snapshot: Snapshot,
  /// index of files in `docs/`, plus a `README.md` from project root
  pub docs: HashMap<String, String>, // 文件相对路径 -> 文档内容
}

/// Global state manager for the MCP server
#[derive(Clone)]
pub struct StateManager {
  /// Current module snapshot (readable and writable)
  current_module: Arc<RwLock<Option<Snapshot>>>,
  /// Dependency modules cache, key is namespace (root namespace of the package)
  dependency_cache: Arc<RwLock<HashMap<String, DepModuleWithDoc>>>,
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
      if let Some(module_with_doc) = cache_guard.get(namespace) {
        return Ok(module_with_doc.snapshot.clone());
      }
    }

    // Load from file
    let module_with_doc = self.load_dependency_from_file(namespace)?;

    // Cache it
    {
      let mut cache_guard = self
        .dependency_cache
        .write()
        .map_err(|e| format!("Failed to write dependency cache lock: {e}"))?;
      cache_guard.insert(namespace.to_string(), module_with_doc.clone());
    }

    Ok(module_with_doc.snapshot)
  }

  /// Get a dependency module with documentation by namespace
  pub fn get_dependency_module_with_doc(&self, namespace: &str) -> Result<DepModuleWithDoc, String> {
    // First check cache by namespace
    {
      let cache_guard = self
        .dependency_cache
        .read()
        .map_err(|e| format!("Failed to read dependency cache lock: {e}"))?;
      if let Some(module_with_doc) = cache_guard.get(namespace) {
        return Ok(module_with_doc.clone());
      }
    }

    // If not found in cache, we need to find the module by folder name
    // For now, assume namespace equals folder name (this is a simplification)
    // In a real implementation, you might want to scan all folders or maintain a mapping
    let module_with_doc = self.load_dependency_from_file(namespace)?;

    // Cache it using the real package namespace from the loaded snapshot
    {
      let mut cache_guard = self
        .dependency_cache
        .write()
        .map_err(|e| format!("Failed to write dependency cache lock: {e}"))?;
      cache_guard.insert(module_with_doc.package.clone(), module_with_doc.clone());
    }

    Ok(module_with_doc)
  }

  /// Load a dependency module by folder name (used for initial loading)
  pub fn load_dependency_by_folder(&self, folder_name: &str) -> Result<DepModuleWithDoc, String> {
    let module_with_doc = self.load_dependency_from_file(folder_name)?;

    // Cache it using the real package namespace
    {
      let mut cache_guard = self
        .dependency_cache
        .write()
        .map_err(|e| format!("Failed to write dependency cache lock: {e}"))?;
      cache_guard.insert(module_with_doc.package.clone(), module_with_doc.clone());
    }

    Ok(module_with_doc)
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
  fn load_dependency_from_file(&self, module_folder_name: &str) -> Result<DepModuleWithDoc, String> {
    // Load dependency module from file system using folder name
    // The folder name may differ from the actual package namespace in the snapshot
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let modules_base_folder = Path::new(&home_dir).join(".config/calcit/modules");

    // Try to find the module file using folder name
    let module_path = modules_base_folder.join(format!("{module_folder_name}/compact.cirru"));

    if module_path.exists() {
      let snapshot = self.load_snapshot_from_file(&module_path.display().to_string())?;
      
      // Get the real package namespace from the snapshot
      let real_package_namespace = snapshot.package.clone();

      // Load documentation files from the docs directory
      let docs_folder = modules_base_folder.join(format!("{module_folder_name}/docs"));
      let mut docs = self.load_docs_from_folder(&docs_folder)?;

      // Load README.md from project root if it exists
      let readme_path = modules_base_folder.join(format!("{module_folder_name}/README.md"));
      if readme_path.exists() {
        match std::fs::read_to_string(&readme_path) {
          Ok(content) => {
            docs.insert("README.md".to_string(), content);
          }
          Err(e) => {
            // Log warning but don't fail the entire operation
            eprintln!("Warning: Failed to read README.md for {module_folder_name}: {e}");
          }
        }
      }

      Ok(DepModuleWithDoc {
        package: real_package_namespace,
        module_folder: module_folder_name.to_string(),
        snapshot,
        docs,
      })
    } else {
      Err(format!("Dependency module not found: {module_folder_name}"))
    }
  }

  /// Load documentation files from a directory
  fn load_docs_from_folder(&self, docs_folder: &Path) -> Result<HashMap<String, String>, String> {
    let mut docs = HashMap::new();

    if !docs_folder.exists() {
      return Ok(docs); // Return empty docs if folder doesn't exist
    }

    fn visit_docs_dir(dir: &Path, base_path: &Path, docs: &mut HashMap<String, String>) -> Result<(), String> {
      let entries = std::fs::read_dir(dir).map_err(|e| format!("Failed to read docs directory {}: {}", dir.display(), e))?;

      for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();

        if path.is_file() {
          // Only process text files (md, txt, etc.)
          if let Some(extension) = path.extension() {
            if matches!(extension.to_str(), Some("md") | Some("txt") | Some("rst") | Some("adoc")) {
              let relative_path = path
                .strip_prefix(base_path)
                .map_err(|e| format!("Failed to get relative path: {e}"))?
                .to_string_lossy()
                .to_string();

              let content = std::fs::read_to_string(&path).map_err(|e| format!("Failed to read doc file {}: {}", path.display(), e))?;

              docs.insert(relative_path, content);
            }
          }
        } else if path.is_dir() {
          // Recursively visit subdirectories
          visit_docs_dir(&path, base_path, docs)?;
        }
      }

      Ok(())
    }

    visit_docs_dir(docs_folder, docs_folder, &mut docs)?;
    Ok(docs)
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

  #[test]
  fn test_module_with_doc_structure() {
    let mut docs = HashMap::new();
    docs.insert("README.md".to_string(), "# Test Module\nThis is a test".to_string());
    docs.insert("docs/guide.md".to_string(), "# Guide\nHow to use this module".to_string());

    let module_with_doc = DepModuleWithDoc {
      package: "test.package".to_string(),
      module_folder: "test-package".to_string(),
      snapshot: Snapshot::default(),
      docs,
    };

    assert_eq!(module_with_doc.package, "test.package");
    assert_eq!(module_with_doc.docs.len(), 2);
    assert!(module_with_doc.docs.contains_key("README.md"));
    assert!(module_with_doc.docs.contains_key("docs/guide.md"));
  }
}
