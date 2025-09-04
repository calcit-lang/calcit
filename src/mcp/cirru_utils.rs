use crate::snapshot::Snapshot;
use cirru_edn::Edn;
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
/// Uses serde for direct conversion since Cirru implements Serialize/Deserialize
pub fn json_to_cirru(value: &JsonValue) -> Result<Cirru, String> {
  serde_json::from_value(value.clone()).map_err(|e| format!("Failed to convert JSON to Cirru: {e}"))
}

/// Convert Cirru structure to JSON value
/// Uses serde for direct conversion since Cirru implements Serialize/Deserialize
pub fn cirru_to_json(cirru: &Cirru) -> JsonValue {
  serde_json::to_value(cirru).unwrap_or(JsonValue::Null)
}

/// Save snapshot to compact.cirru file
/// This is a shared utility function to avoid code duplication across handlers
pub fn save_snapshot_to_file<P: AsRef<Path>>(compact_cirru_path: P, snapshot: &Snapshot) -> Result<(), String> {
  // Build root level Edn mapping
  let mut edn_map = cirru_edn::EdnMapView::default();

  // Build package
  edn_map.insert_key("package", Edn::Str(snapshot.package.as_str().into()));

  // Build docs
  if let Some(docs) = &snapshot.docs {
    let mut docs_map = cirru_edn::EdnMapView::default();
    for (key, doc_entry) in docs {
      docs_map.insert_key(key.as_str(), Edn::from(doc_entry));
    }
    edn_map.insert_key("docs", docs_map.into());
  }

  // Build configs
  let mut configs_map = cirru_edn::EdnMapView::default();
  configs_map.insert_key("init-fn", Edn::Str(snapshot.configs.init_fn.as_str().into()));
  configs_map.insert_key("reload-fn", Edn::Str(snapshot.configs.reload_fn.as_str().into()));
  configs_map.insert_key("version", Edn::Str(snapshot.configs.version.as_str().into()));
  configs_map.insert_key(
    "modules",
    Edn::from(
      snapshot
        .configs
        .modules
        .iter()
        .map(|s| Edn::Str(s.as_str().into()))
        .collect::<Vec<_>>(),
    ),
  );
  edn_map.insert_key("configs", configs_map.into());

  // Build entries
  let mut entries_map = cirru_edn::EdnMapView::default();
  for (k, v) in &snapshot.entries {
    let mut entry_map = cirru_edn::EdnMapView::default();
    entry_map.insert_key("init-fn", Edn::Str(v.init_fn.as_str().into()));
    entry_map.insert_key("reload-fn", Edn::Str(v.reload_fn.as_str().into()));
    entry_map.insert_key("version", Edn::Str(v.version.as_str().into()));
    entry_map.insert_key(
      "modules",
      Edn::from(v.modules.iter().map(|s| Edn::Str(s.as_str().into())).collect::<Vec<_>>()),
    );
    entries_map.insert_key(k.as_str(), entry_map.into());
  }
  edn_map.insert_key("entries", entries_map.into());

  // Build files
  let mut files_map = cirru_edn::EdnMapView::default();
  for (k, v) in &snapshot.files {
    // Skip $meta namespaces as they are special and should not be serialized to file
    if k.ends_with(".$meta") {
      continue;
    }
    files_map.insert(Edn::str(k.as_str()), Edn::from(v));
  }
  edn_map.insert_key("files", files_map.into());

  let edn_data = Edn::from(edn_map);

  // Format Edn as Cirru string
  let content = cirru_edn::format(&edn_data, true).map_err(|e| format!("Failed to format snapshot as Cirru: {e}"))?;

  // Write to file
  std::fs::write(compact_cirru_path, content).map_err(|e| format!("Failed to write compact.cirru: {e}"))?;

  Ok(())
}
