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
  pub docs: Vec<String>, // 文件相对路径列表
}

impl DepModuleWithDoc {
  /// 动态读取文档文件内容
  pub fn read_doc_content(&self, doc_path: &str) -> Result<String, String> {
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let modules_base_folder = Path::new(&home_dir).join(".config/calcit/modules");

    let file_path = if doc_path == "README.md" {
      // README.md 在项目根目录
      modules_base_folder.join(format!("{}/README.md", self.module_folder))
    } else {
      // 其他文档在 docs/ 目录下
      modules_base_folder.join(format!("{}/docs/{}", self.module_folder, doc_path))
    };

    std::fs::read_to_string(&file_path).map_err(|e| format!("Failed to read doc file {}: {}", file_path.display(), e))
  }

  /// 获取所有文档文件路径
  pub fn get_doc_paths(&self) -> &Vec<String> {
    &self.docs
  }
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

  /// Ensure all dependency modules are loaded into cache
  /// This method reads the modules configuration from the current snapshot
  /// and loads all dependency modules into the dependency_cache
  pub fn ensure_dependencies_loaded(&self) -> Result<(), String> {
    // First ensure current module is loaded
    self.ensure_current_module_loaded()?;

    // Always load the built-in calcit-core.cirru module first
    {
      let cache_guard = self
        .dependency_cache
        .read()
        .map_err(|e| format!("Failed to read dependency cache lock: {e}"))?;

      // Check if calcit is already loaded (the actual package name in calcit-core.cirru)
      if !cache_guard.contains_key("calcit") {
        drop(cache_guard);
        
        // Load the core module
        match self.load_core_module() {
          Ok(core_module) => {
            let mut cache_guard = self
              .dependency_cache
              .write()
              .map_err(|e| format!("Failed to write dependency cache lock: {e}"))?;
            cache_guard.insert(core_module.package.clone(), core_module);
            println!("[DEPS] Loaded built-in calcit-core.cirru module");
          }
          Err(e) => {
            println!("[DEPS] Warning: Failed to load built-in calcit-core.cirru module: {e}");
          }
        }
      }
    }

    // Get the modules list from current snapshot
    let modules = {
      let current_guard = self
        .current_module
        .read()
        .map_err(|e| format!("Failed to read current module lock: {e}"))?;

      if let Some(ref snapshot) = *current_guard {
        snapshot.configs.modules.clone()
      } else {
        return Err("Current module not loaded".to_string());
      }
    };

    // Check if all modules are already loaded to avoid redundant loading
    {
      let cache_guard = self
        .dependency_cache
        .read()
        .map_err(|e| format!("Failed to read dependency cache lock: {e}"))?;

      // Extract module folder names from configuration
      let config_module_folders: std::collections::HashSet<&str> = modules
        .iter()
        .map(|path| {
          if path.ends_with('/') {
            path.trim_end_matches('/')
          } else {
            path.as_str()
          }
        })
        .collect();

      // Get cached module folders
      let cached_module_folders: std::collections::HashSet<&str> = cache_guard.values().map(|dep| dep.module_folder.as_str()).collect();

      // If all configured modules are already cached, skip loading
      if !config_module_folders.is_empty() && config_module_folders.is_subset(&cached_module_folders) {
        println!("[DEPS] All dependency modules already loaded, skipping reload");
        return Ok(());
      }
    }

    // Load each module into dependency cache if not already loaded
    for module_path in &modules {
      // Check if module is already in cache
      let cache_guard = self
        .dependency_cache
        .read()
        .map_err(|e| format!("Failed to read dependency cache lock: {e}"))?;

      // Extract module folder name from path
      let module_folder = if module_path.ends_with('/') {
        module_path.trim_end_matches('/')
      } else {
        module_path
      };

      // Check if any cached module has this folder name
      let already_cached = cache_guard.values().any(|dep| dep.module_folder == module_folder);
      drop(cache_guard);

      if !already_cached {
        // Load the dependency module
        match self.load_dependency_by_folder(module_folder) {
          Ok(_) => {
            println!("[DEPS] Loaded dependency module: {module_folder}");
          }
          Err(e) => {
            println!("[DEPS] Warning: Failed to load dependency module '{module_folder}': {e}");
            // Continue loading other modules even if one fails
          }
        }
      }
    }

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

  /// Get all dependency namespaces from cached dependencies
  /// This method ensures all dependencies are loaded before returning namespaces
  pub fn get_dependency_namespaces(&self) -> Result<Vec<String>, String> {
    // Ensure all dependencies are loaded first
    self.ensure_dependencies_loaded()?;

    let cache_guard = self
      .dependency_cache
      .read()
      .map_err(|e| format!("Failed to read dependency cache lock: {e}"))?;

    let mut namespaces = Vec::new();
    for (_, dep_module) in cache_guard.iter() {
      for namespace in dep_module.snapshot.files.keys() {
        namespaces.push(namespace.clone());
      }
    }

    Ok(namespaces)
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

  /// Find a namespace in dependency modules and return the module info and file data
  pub fn find_namespace_in_dependencies(&self, namespace: &str) -> Result<Option<(String, crate::snapshot::FileInSnapShot)>, String> {
    let cache_guard = self
      .dependency_cache
      .read()
      .map_err(|e| format!("Failed to read dependency cache lock: {e}"))?;

    for (package_name, dep_module) in cache_guard.iter() {
      if let Some(file_data) = dep_module.snapshot.files.get(namespace) {
        return Ok(Some((package_name.clone(), file_data.clone())));
      }
    }

    Ok(None)
  }

  /// Get all available dependency namespaces with their package names
  pub fn get_all_dependency_namespaces(&self) -> Result<Vec<(String, String)>, String> {
    let cache_guard = self
      .dependency_cache
      .read()
      .map_err(|e| format!("Failed to read dependency cache lock: {e}"))?;

    let mut namespaces = Vec::new();
    for (package_name, dep_module) in cache_guard.iter() {
      for namespace in dep_module.snapshot.files.keys() {
        namespaces.push((namespace.clone(), package_name.clone()));
      }
    }

    Ok(namespaces)
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

  /// Load the built-in calcit-core.cirru module as DepModuleWithDoc
  fn load_core_module(&self) -> Result<DepModuleWithDoc, String> {
    // Load the embedded calcit-core.cirru content
    let bytes = include_bytes!("../cirru/calcit-core.cirru");
    let core_content = String::from_utf8_lossy(bytes).to_string();
    let core_data = cirru_edn::parse(&core_content).map_err(|e| format!("Failed to parse calcit-core.cirru: {e}"))?;
    let snapshot = crate::snapshot::load_snapshot_data(&core_data, "calcit-internal://calcit-core.cirru")?;

    Ok(DepModuleWithDoc {
      package: snapshot.package.clone(), // Use the actual package name from the file ("calcit")
      module_folder: "calcit-core".to_string(),
      snapshot,
      docs: vec![], // No external docs for built-in module
    })
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

      // Check if README.md exists in project root
      let readme_path = modules_base_folder.join(format!("{module_folder_name}/README.md"));
      if readme_path.exists() {
        docs.push("README.md".to_string());
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

  /// Load documentation file paths from a directory
  fn load_docs_from_folder(&self, docs_folder: &Path) -> Result<Vec<String>, String> {
    let mut docs = Vec::new();

    if !docs_folder.exists() {
      return Ok(docs); // Return empty docs if folder doesn't exist
    }

    fn visit_docs_dir(dir: &Path, base_path: &Path, docs: &mut Vec<String>) -> Result<(), String> {
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

              docs.push(relative_path);
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
    let state_manager = StateManager::new("/tmp/test_compact.cirru".to_string());
    // Test that dependency cache starts empty
    let cache_guard = state_manager.dependency_cache.read().unwrap();
    assert!(cache_guard.is_empty(), "Dependency cache should start empty");
  }

  #[test]
  fn test_ensure_dependencies_loaded() {
    use crate::snapshot::{Snapshot, SnapshotConfigs};
    use std::collections::HashMap;
    use std::fs;

    let test_file = "/tmp/test_deps_loaded.cirru";
    let state_manager = StateManager::new(test_file.to_string());

    // Create a test snapshot with modules configuration
    let snapshot = Snapshot {
      package: "test-package".to_string(),
      configs: SnapshotConfigs {
        init_fn: "test.init".to_string(),
        reload_fn: "test.reload".to_string(),
        version: "0.1.0".to_string(),
        modules: vec!["test-module".to_string()],
      },
      entries: HashMap::new(),
      files: HashMap::new(),
    };

    // Manually set the current module
    {
      let mut current_guard = state_manager.current_module.write().unwrap();
      *current_guard = Some(snapshot.clone());
    }

    // Test ensure_dependencies_loaded method
    // Note: This will fail to load actual modules since they don't exist,
    // but it should not panic and should handle errors gracefully
    let result = state_manager.ensure_dependencies_loaded();
    assert!(
      result.is_ok(),
      "ensure_dependencies_loaded should handle missing modules gracefully"
    );

    // Clean up
    let _ = fs::remove_file(test_file);
  }

  #[test]
  fn test_module_with_doc_structure() {
    let docs = vec!["README.md".to_string(), "docs/guide.md".to_string()];

    let module_with_doc = DepModuleWithDoc {
      package: "test.package".to_string(),
      module_folder: "test-package".to_string(),
      snapshot: Snapshot::default(),
      docs,
    };

    assert_eq!(module_with_doc.package, "test.package");
    assert_eq!(module_with_doc.docs.len(), 2);
    assert!(module_with_doc.docs.contains(&"README.md".to_string()));
    assert!(module_with_doc.docs.contains(&"docs/guide.md".to_string()));
  }

  #[test]
  fn test_calcit_core_module_loading() {
    let state_manager = StateManager::new("dummy.cirru".to_string());
    
    // Test loading calcit-core.cirru module
    let core_module_result = state_manager.load_core_module();
    assert!(core_module_result.is_ok(), "Failed to load calcit-core.cirru module: {:?}", core_module_result.err());
    
    let core_module = core_module_result.unwrap();
    assert_eq!(core_module.package, "calcit"); // The actual package name in calcit-core.cirru
    assert_eq!(core_module.module_folder, "calcit-core");
    assert!(!core_module.snapshot.files.is_empty(), "calcit-core snapshot should have files");
    
    // Check if calcit.core file exists in the snapshot
    assert!(core_module.snapshot.files.contains_key("calcit.core"), 
            "calcit.core file should exist in snapshot");
    
    // Check if there are some definitions in calcit.core
    if let Some(core_file) = core_module.snapshot.files.get("calcit.core") {
      assert!(!core_file.defs.is_empty(), "calcit.core should have definitions");
      println!("calcit.core has {} definitions", core_file.defs.len());
      
      // Check for some common definitions
      let common_defs = ["map", "filter", "+", "-"];
      for def_name in &common_defs {
        if core_file.defs.contains_key(*def_name) {
          println!("Found definition: {def_name}");
        }
      }
    }
  }

  #[test]
  fn test_calcit_core_dependency_loading() {
    let state_manager = StateManager::new("dummy.cirru".to_string());
    
    // Manually load calcit into cache (simulating what ensure_dependencies_loaded does)
    let core_module = state_manager.load_core_module().expect("Failed to load calcit-core.cirru module");
    {
      let mut cache_guard = state_manager.dependency_cache.write().expect("Failed to get cache lock");
      cache_guard.insert(core_module.package.clone(), core_module.clone());
    }
    
    // Test querying calcit as a dependency (the actual package name)
    let core_snapshot_result = state_manager.get_dependency_module("calcit");
    assert!(core_snapshot_result.is_ok(), "Failed to get calcit as dependency: {:?}", core_snapshot_result.err());
    
    let core_snapshot = core_snapshot_result.unwrap();
    assert_eq!(core_snapshot.package, "calcit");
    assert!(core_snapshot.files.contains_key("calcit.core"));
    
    // Test querying calcit with docs
    let core_with_docs_result = state_manager.get_dependency_module_with_doc("calcit");
    assert!(core_with_docs_result.is_ok(), "Failed to get calcit with docs: {:?}", core_with_docs_result.err());
    
    let core_with_docs = core_with_docs_result.unwrap();
    assert_eq!(core_with_docs.package, "calcit");
    assert_eq!(core_with_docs.module_folder, "calcit-core");
    
    // Test that we can query specific definitions
    if let Some(core_file) = core_snapshot.files.get("calcit.core") {
      // Test some common definitions
      assert!(core_file.defs.contains_key("map"), "Should contain 'map' definition");
      assert!(core_file.defs.contains_key("+"), "Should contain '+' definition");
      
      // Test that definitions exist and can be accessed
      if let Some(map_def) = core_file.defs.get("map") {
        println!("map definition found with doc: '{}'", map_def.doc);
        // Note: calcit-core.cirru definitions may have empty docs, which is normal
      }
      
      if let Some(plus_def) = core_file.defs.get("+") {
        println!("+ definition found with doc: '{}'", plus_def.doc);
      }
      
      println!("Successfully verified that calcit.core definitions can be queried");
    }
  }

  #[test]
  fn test_calcit_core_in_namespaces() {
    let state_manager = StateManager::new("dummy.cirru".to_string());
    
    // Manually load calcit into cache
    let core_module = state_manager.load_core_module().expect("Failed to load calcit-core.cirru module");
    {
      let mut cache_guard = state_manager.dependency_cache.write().expect("Failed to get cache lock");
      cache_guard.insert(core_module.package.clone(), core_module.clone());
    }
    
    // Test that calcit.core namespace appears in dependency namespaces
    // We'll manually extract namespaces from cache to avoid calling ensure_dependencies_loaded
    let cache_guard = state_manager.dependency_cache.read().expect("Failed to get cache lock");
    let mut namespaces = Vec::new();
    for (_, dep_module) in cache_guard.iter() {
      for namespace in dep_module.snapshot.files.keys() {
        namespaces.push(namespace.clone());
      }
    }
    drop(cache_guard);
    
    println!("Available dependency namespaces: {namespaces:?}");
    
    // Check that calcit.core namespace is available
    assert!(namespaces.contains(&"calcit.core".to_string()), 
            "calcit.core namespace should be available in dependency namespaces");
    
    println!("Successfully verified that calcit.core appears in dependency namespaces");
  }

  #[test]
  fn test_mcp_definition_query() {
    let state_manager = StateManager::new("dummy.cirru".to_string());
    
    // Manually load calcit into cache (simulating what ensure_dependencies_loaded does)
    let core_module = state_manager.load_core_module().expect("Failed to load calcit-core.cirru module");
    {
      let mut cache_guard = state_manager.dependency_cache.write().expect("Failed to get cache lock");
      cache_guard.insert(core_module.package.clone(), core_module.clone());
    }
    
    // Test querying specific definitions from calcit.core (simulating MCP tool behavior)
    
    // 1. Test querying the map function
    let map_query_result = state_manager.get_dependency_module("calcit");
    assert!(map_query_result.is_ok(), "Failed to get calcit dependency: {:?}", map_query_result.err());
    
    let calcit_snapshot = map_query_result.unwrap();
    assert!(calcit_snapshot.files.contains_key("calcit.core"), "calcit.core file should exist");
    
    if let Some(core_file) = calcit_snapshot.files.get("calcit.core") {
      // Test that we can find the map definition
      assert!(core_file.defs.contains_key("map"), "map definition should exist in calcit.core");
      
      if let Some(map_def) = core_file.defs.get("map") {
         println!("Found map definition:");
         println!("  Doc: '{}'", map_def.doc);
         
         // The definition should have some code content
         println!("  Code: {:?}", map_def.code);
       }
      
      // Test other common definitions
      let test_defs = ["+", "-", "*", "/", "filter", "reduce"];
      for def_name in &test_defs {
        if let Some(def) = core_file.defs.get(*def_name) {
          println!("Found definition '{}' with doc: '{}'", def_name, def.doc);
        }
      }
      
      println!("Total definitions in calcit.core: {}", core_file.defs.len());
    }
    
    // 2. Test querying with documentation (simulating read_dependency_definition_doc)
    let core_with_docs_result = state_manager.get_dependency_module_with_doc("calcit");
    assert!(core_with_docs_result.is_ok(), "Failed to get calcit with docs: {:?}", core_with_docs_result.err());
    
    let core_with_docs = core_with_docs_result.unwrap();
    println!("Package: {}, Module folder: {}", core_with_docs.package, core_with_docs.module_folder);
    println!("Available doc files: {:?}", core_with_docs.docs);
    
    // Verify that the snapshot contains the same definitions
    if let Some(core_file) = core_with_docs.snapshot.files.get("calcit.core") {
      assert!(core_file.defs.contains_key("map"), "map should be queryable through doc interface");
      println!("Successfully verified MCP-style definition queries work for calcit.core");
    }
  }

  #[test]
  fn test_list_namespace_definitions_dependency() {
    let state_manager = StateManager::new("dummy.cirru".to_string());

    // Manually load calcit module to cache
    let core_module = state_manager.load_core_module().expect("Failed to load core module");
    {
      let mut cache_guard = state_manager.dependency_cache.write().expect("Failed to get write lock");
      cache_guard.insert(core_module.package.clone(), core_module.clone());
    }

    // Test that we can get the dependency module for calcit package
    let dep_module_result = state_manager.get_dependency_module("calcit");
    assert!(dep_module_result.is_ok(), "Should be able to get dependency module for calcit package");

    let dep_module = dep_module_result.unwrap();
    assert!(dep_module.files.contains_key("calcit.core"), "Dependency module should contain calcit.core namespace");

    let calcit_core_file = &dep_module.files["calcit.core"];
    let definitions: Vec<String> = calcit_core_file.defs.keys().cloned().collect();

    assert!(!definitions.is_empty(), "Should have definitions in calcit.core");
    assert!(definitions.contains(&"map".to_string()), "Should contain 'map' definition");
    assert!(definitions.contains(&"+".to_string()), "Should contain '+' definition");

    println!("Successfully found {} definitions in calcit.core dependency", definitions.len());
    println!("Sample definitions: {:?}", &definitions[..std::cmp::min(10, definitions.len())]);
  }

  #[test]
  fn test_mcp_list_namespace_definitions_dependency() {
    use crate::mcp::read_handlers::list_namespace_definitions;
    use crate::mcp::tools::ListDefinitionsRequest;
    use crate::mcp::AppState;


    let state_manager = StateManager::new("dummy.cirru".to_string());

    // Manually load calcit module to cache
    let core_module = state_manager.load_core_module().expect("Failed to load core module");
    {
      let mut cache_guard = state_manager.dependency_cache.write().expect("Failed to get write lock");
      cache_guard.insert(core_module.package.clone(), core_module.clone());
    }

    let app_state = AppState {
      compact_cirru_path: "dummy.cirru".to_string(),
      current_module_name: "test".to_string(),
      port: 8080,
      state_manager,
    };

    // Test list_namespace_definitions for calcit.core
    let request = ListDefinitionsRequest {
      namespace: "calcit.core".to_string(),
    };

    let response = list_namespace_definitions(&app_state, request);
    let response_value = &response.0;

    // Check if the response is successful
    if let Some(error) = response_value.get("error") {
      panic!("list_namespace_definitions failed: {error}");
    }

    // Verify response structure
    assert!(response_value.get("namespace").is_some(), "Response should contain namespace");
    assert!(response_value.get("definitions").is_some(), "Response should contain definitions");
    assert!(response_value.get("source").is_some(), "Response should contain source");

    let namespace = response_value["namespace"].as_str().unwrap();
    let definitions = response_value["definitions"].as_array().unwrap();
    let source = response_value["source"].as_str().unwrap();

    assert_eq!(namespace, "calcit.core");
    assert_eq!(source, "dependency");
    assert!(!definitions.is_empty(), "Should have definitions in calcit.core");

    // Check for some common definitions
    let definition_names: Vec<&str> = definitions
      .iter()
      .map(|v| v.as_str().unwrap())
      .collect();

    assert!(definition_names.contains(&"map"), "Should contain 'map' definition");
    assert!(definition_names.contains(&"+"), "Should contain '+' definition");

    println!("Successfully listed {} definitions from calcit.core dependency via MCP", definitions.len());
    println!("Sample definitions: {:?}", &definition_names[..std::cmp::min(10, definition_names.len())]);
  }
}
