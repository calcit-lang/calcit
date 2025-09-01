use crate::snapshot::Snapshot;
use cirru_parser::Cirru;
use serde_json::Value as JsonValue;
use std::path::Path;

/// Validate if JSON value conforms to Cirru recursive structure
pub fn validate_cirru_structure(value: &JsonValue) -> Result<(), String> {
  match value {
    JsonValue::String(_) => Ok(()),
    JsonValue::Array(arr) => {
      for item in arr {
        validate_cirru_structure(item)?;
      }
      Ok(())
    }
    _ => Err("Cirru structure must be strings or arrays only".to_string()),
  }
}

/// Convert JSON value to Cirru structure
pub fn json_to_cirru(value: &JsonValue) -> Result<Cirru, String> {
  match value {
    JsonValue::String(s) => Ok(Cirru::Leaf(s.as_str().into())),
    JsonValue::Array(arr) => {
      let mut cirru_list = Vec::new();
      for item in arr {
        cirru_list.push(json_to_cirru(item)?);
      }
      Ok(Cirru::List(cirru_list))
    }
    _ => Err("Invalid JSON structure for Cirru conversion".to_string()),
  }
}

/// Convert Cirru structure to JSON value
pub fn cirru_to_json(cirru: &Cirru) -> JsonValue {
  match cirru {
    Cirru::Leaf(s) => JsonValue::String(s.to_string()),
    Cirru::List(list) => {
      let json_list: Vec<JsonValue> = list.iter().map(cirru_to_json).collect();
      JsonValue::Array(json_list)
    }
  }
}

/// Save snapshot to compact.cirru file
/// This is a shared utility function to avoid code duplication across handlers
pub fn save_snapshot_to_file<P: AsRef<Path>>(compact_cirru_path: P, snapshot: &Snapshot) -> Result<(), String> {
  // Build root level Edn mapping
  let mut edn_map = cirru_edn::EdnMapView::default();

  // Build package
  edn_map.insert_key("package", cirru_edn::Edn::Str(snapshot.package.as_str().into()));

  // Build configs
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

  // Build entries
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

  // Build files
  let mut files_map = cirru_edn::EdnMapView::default();
  for (k, v) in &snapshot.files {
    files_map.insert_key(k.as_str(), cirru_edn::Edn::from(v));
  }
  edn_map.insert_key("files", files_map.into());

  let edn_data = cirru_edn::Edn::from(edn_map);

  // Format Edn as Cirru string
  let content = cirru_edn::format(&edn_data, false).map_err(|e| format!("Failed to format snapshot as Cirru: {e}"))?;

  // Write to file
  std::fs::write(compact_cirru_path, content).map_err(|e| format!("Failed to write compact.cirru: {e}"))?;

  Ok(())
}
