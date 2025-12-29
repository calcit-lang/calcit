use argh::FromArgs;
use calcit::detailed_snapshot::{
  DetailCirru, DetailedCodeEntry, DetailedFileInSnapshot, DetailedSnapshot, load_detailed_snapshot_data,
};
use calcit::snapshot::{CodeEntry, FileInSnapShot, Snapshot, load_snapshot_data};
use cirru_edn::Edn;
use cirru_parser::Cirru;
use std::fmt::Debug;
use std::fs;
use std::path::PathBuf;

#[derive(FromArgs)]
/// Sync changes from compact.cirru to calcit.cirru while preserving metadata
struct Args {
  #[argh(option, short = 'c', default = "PathBuf::from(\"compact.cirru\")")]
  /// path to compact.cirru file
  compact_path: PathBuf,

  #[argh(option, short = 'f', default = "PathBuf::from(\"calcit.cirru\")")]
  /// path to calcit.cirru file
  calcit_path: PathBuf,

  #[argh(switch, short = 'd')]
  /// dry run mode - show changes without applying them
  dry_run: bool,

  #[argh(switch, short = 'v')]
  /// verbose output
  verbose: bool,
}

/// Print code changes with aligned line headers for easier comparison of differences
fn print_code_change(from: &Cirru, to: &Cirru) {
  println!("    Code change:");
  println!("      From: {from:?}");
  println!("      To:   {to:?}");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let args: Args = argh::from_env();

  if args.verbose {
    println!(
      "Syncing changes from {} to {}",
      args.compact_path.display(),
      args.calcit_path.display()
    );
  }

  // Read both files
  let compact_content = fs::read_to_string(&args.compact_path).map_err(|e| format!("Failed to read compact.cirru: {e}"))?;

  let calcit_content = fs::read_to_string(&args.calcit_path).map_err(|e| format!("Failed to read calcit.cirru: {e}"))?;

  // Parse compact file as Snapshot
  let compact_data = cirru_edn::parse(&compact_content).map_err(|e| format!("Failed to parse compact file: {e}"))?;
  let compact_snapshot = load_snapshot_data(&compact_data, &args.compact_path.to_string_lossy())
    .map_err(|e| format!("Failed to load compact snapshot: {e}"))?;

  // Parse calcit file as DetailedSnapshot
  let calcit_data = cirru_edn::parse(&calcit_content).map_err(|e| format!("Failed to parse calcit file: {e}"))?;
  let mut detailed_snapshot = load_detailed_snapshot_data(&calcit_data, &args.calcit_path.to_string_lossy())
    .map_err(|e| format!("Failed to load detailed snapshot: {e}"))?;

  if args.verbose {
    println!("Loaded compact snapshot with {} files", compact_snapshot.files.len());
    println!("Loaded calcit snapshot with {} files", detailed_snapshot.files.len());
  }

  // Detect changes between the two snapshots
  let changes = detect_snapshot_changes(&compact_snapshot, &detailed_snapshot);

  if changes.is_empty() {
    println!("No changes detected between compact and calcit files.");
    return Ok(());
  }

  if args.verbose {
    println!("Detected {} changes:", changes.len());
    for change in &changes {
      println!("  {change}");

      // 打印差异原因的详细信息
      if let (Some(new_entry), ChangePath::FunctionDefinition { file_name, def_name }) = (&change.new_entry, &change.path) {
        if let Some(detailed_file) = detailed_snapshot.files.get(file_name) {
          if let Some(detailed_entry) = detailed_file.defs.get(def_name) {
            match change.change_type {
              ChangeType::ModifiedCode => {
                let compact_cirru: Cirru = new_entry.code.clone();
                let detailed_cirru: Cirru = detailed_entry.code.clone().into();
                print_code_change(&detailed_cirru, &compact_cirru);
              }
              ChangeType::ModifiedDoc => {
                println!("    Doc change: from \"{}\" to \"{}\"", detailed_entry.doc, new_entry.doc);
              }
              ChangeType::Modified => {
                let compact_cirru: Cirru = new_entry.code.clone();
                let detailed_cirru: Cirru = detailed_entry.code.clone().into();
                print_code_change(&detailed_cirru, &compact_cirru);
                println!("    Doc change: from \"{}\" to \"{}\"", detailed_entry.doc, new_entry.doc);
              }
              _ => {}
            }
          }
        }
      } else if let (Some(new_entry), ChangePath::NamespaceDefinition { file_name }) = (&change.new_entry, &change.path) {
        if let Some(detailed_file) = detailed_snapshot.files.get(file_name) {
          match change.change_type {
            ChangeType::ModifiedCode => {
              let compact_cirru: Cirru = new_entry.code.clone();
              let detailed_cirru: Cirru = detailed_file.ns.code.clone().into();
              print_code_change(&detailed_cirru, &compact_cirru);
            }
            ChangeType::ModifiedDoc => {
              println!("    Doc change: from \"{}\" to \"{}\"", detailed_file.ns.doc, new_entry.doc);
            }
            ChangeType::Modified => {
              let compact_cirru: Cirru = new_entry.code.clone();
              let detailed_cirru: Cirru = detailed_file.ns.code.clone().into();
              print_code_change(&detailed_cirru, &compact_cirru);
              println!("    Doc change: from \"{}\" to \"{}\"", detailed_file.ns.doc, new_entry.doc);
            }
            _ => {}
          }
        }
      }
    }
  }

  if args.dry_run {
    println!("Dry run mode: would apply {} changes", changes.len());
    return Ok(());
  }

  // Apply changes to detailed snapshot
  apply_snapshot_changes(&mut detailed_snapshot, &changes);

  // Convert DetailedSnapshot back to Edn for saving
  let updated_edn = detailed_snapshot_to_edn(&detailed_snapshot);
  let formatted_content = cirru_edn::format(&updated_edn, true).map_err(|e| format!("Failed to format updated calcit.cirru: {e}"))?;

  fs::write(&args.calcit_path, formatted_content).map_err(|e| format!("Failed to write calcit.cirru: {e}"))?;

  println!("Successfully applied {} changes to {}", changes.len(), args.calcit_path.display());

  Ok(())
}

#[derive(Debug, Clone)]
enum ChangePath {
  NamespaceDefinition { file_name: String },
  FunctionDefinition { file_name: String, def_name: String },
}

impl std::fmt::Display for ChangePath {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ChangePath::NamespaceDefinition { file_name } => write!(f, "{file_name}.ns"),
      ChangePath::FunctionDefinition { file_name, def_name } => write!(f, "{file_name}.defs.{def_name}"),
    }
  }
}

#[derive(Debug, Clone)]
struct SnapshotChange {
  path: ChangePath,
  change_type: ChangeType,
  new_entry: Option<CodeEntry>,
}

#[derive(Debug, Clone, PartialEq)]
enum ChangeType {
  Added,
  Modified,
  ModifiedCode,
  ModifiedDoc,
  Removed,
}

impl std::fmt::Display for SnapshotChange {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match &self.change_type {
      ChangeType::Added => write!(f, "Added: {}", self.path),
      ChangeType::Modified => write!(f, "Modified: {}", self.path),
      ChangeType::ModifiedCode => write!(f, "Modified code: {}", self.path),
      ChangeType::ModifiedDoc => write!(f, "Modified doc: {}", self.path),
      ChangeType::Removed => write!(f, "Removed: {}", self.path),
    }
  }
}

fn detailed_snapshot_to_edn(snapshot: &DetailedSnapshot) -> Edn {
  // Create a modified snapshot, handling special cases
  let mut modified_snapshot = snapshot.clone();

  // Filter out $meta namespaces
  modified_snapshot.files.retain(|k, _| !k.ends_with(".$meta"));

  // Fix empty or unknown namespace declarations
  for (file_name, file) in &mut modified_snapshot.files {
    let ns_cirru: Cirru = file.ns.code.clone().into();
    if ns_cirru == vec!["ns", "unknown"].into() || ns_cirru.is_empty() {
      let fixed_cirru: Cirru = vec!["ns", file_name.as_str()].into();
      file.ns.code = fixed_cirru.into();
    }
  }
  // Manually build EDN structure for DetailedSnapshot
  let mut edn_map = cirru_edn::EdnMapView::default();

  // Add package
  edn_map.insert_key("package", Edn::Str(modified_snapshot.package.as_str().into()));

  // Add configs
  edn_map.insert_key("configs", modified_snapshot.configs.clone());

  // Add entries
  edn_map.insert_key("entries", modified_snapshot.entries.clone());

  // Add files
  let mut files_map = cirru_edn::EdnMapView::default();
  for (k, v) in &modified_snapshot.files {
    // Skip $meta namespaces as they are special
    if k.ends_with(".$meta") {
      continue;
    }

    let mut file_record = cirru_edn::EdnRecordView {
      tag: cirru_edn::EdnTag::new("FileEntry"),
      pairs: Vec::new(),
    };

    // Add defs field
    let mut defs_map = cirru_edn::EdnMapView::default();
    for (def_name, def_entry) in &v.defs {
      defs_map.insert(Edn::str(def_name.as_str()), detailed_code_entry_to_edn(def_entry));
    }
    file_record.pairs.push(("defs".into(), Edn::from(defs_map)));

    // Add ns field, make sure "ns" is after "defs"
    file_record.pairs.push(("ns".into(), detailed_code_entry_to_edn(&v.ns)));

    files_map.insert(Edn::str(k.as_str()), Edn::Record(file_record));
  }
  edn_map.insert_key("files", files_map.into());

  // Add users if present
  if !modified_snapshot.users.is_nil() {
    edn_map.insert_key("users", modified_snapshot.users.clone());
  }

  Edn::from(edn_map)
}

// Helper function to convert DetailedCodeEntry to Edn
fn detailed_code_entry_to_edn(entry: &DetailedCodeEntry) -> Edn {
  let mut record = cirru_edn::EdnRecordView {
    tag: cirru_edn::EdnTag::new("CodeEntry"),
    pairs: Vec::new(),
  };

  // Add code field (must be first based on Calcit's expectation)
  record.pairs.push(("code".into(), detailed_cirru_to_edn(&entry.code)));

  // Add doc field
  record.pairs.push(("doc".into(), Edn::Str(entry.doc.as_str().into())));

  // Add examples field - convert DetailCirru to simple Cirru first (without metadata)
  let examples_list: Vec<Edn> = entry
    .examples
    .iter()
    .map(|e| {
      let simple_cirru: Cirru = e.clone().into();
      simple_cirru.into()
    })
    .collect();
  record.pairs.push(("examples".into(), Edn::List(examples_list.into())));

  Edn::Record(record)
}

// Helper function to convert DetailCirru to Edn
fn detailed_cirru_to_edn(cirru: &DetailCirru) -> Edn {
  match cirru {
    DetailCirru::Leaf { at, by, text } => {
      let mut record = cirru_edn::EdnRecordView {
        tag: cirru_edn::EdnTag::new("Leaf"),
        pairs: Vec::new(),
      };

      record.pairs.push(("at".into(), Edn::Number(*at as f64)));
      record.pairs.push(("by".into(), Edn::Str(by.as_str().into())));

      if let Some(t) = text {
        record.pairs.push(("text".into(), Edn::Str(t.as_str().into())));
      }

      Edn::Record(record)
    }
    DetailCirru::List { data, at, by } => {
      let mut record = cirru_edn::EdnRecordView {
        tag: cirru_edn::EdnTag::new("Expr"), // align with old implementation
        pairs: Vec::new(),
      };

      record.pairs.push(("at".into(), Edn::Number(*at as f64)));
      record.pairs.push(("by".into(), Edn::Str(by.as_str().into())));

      let mut data_map = cirru_edn::EdnMapView::default();
      for (k, v) in data {
        data_map.insert(Edn::str(k.as_str()), detailed_cirru_to_edn(v));
      }
      record.pairs.push(("data".into(), Edn::from(data_map)));

      Edn::Record(record)
    }
  }
}

fn detect_snapshot_changes(compact: &Snapshot, detailed: &DetailedSnapshot) -> Vec<SnapshotChange> {
  let mut changes = Vec::new();

  // Compare files
  for (file_name, compact_file) in &compact.files {
    // Skip $meta namespaces
    if file_name.ends_with(".$meta") {
      continue;
    }

    match detailed.files.get(file_name) {
      Some(detailed_file) => {
        // File exists in both, compare definitions
        compare_file_definitions(file_name, compact_file, detailed_file, &mut changes);
      }
      None => {
        // File added in compact: add namespace first to preserve require rules
        changes.push(SnapshotChange {
          path: ChangePath::NamespaceDefinition {
            file_name: file_name.clone(),
          },
          change_type: ChangeType::Added,
          new_entry: Some(compact_file.ns.clone()),
        });

        // Then add all definitions in the new file
        for (def_name, code_entry) in &compact_file.defs {
          changes.push(SnapshotChange {
            path: ChangePath::FunctionDefinition {
              file_name: file_name.clone(),
              def_name: def_name.clone(),
            },
            change_type: ChangeType::Added,
            new_entry: Some(code_entry.clone()),
          });
        }
      }
    }
  }

  // Check for removed files
  for (file_name, detailed_file) in &detailed.files {
    if !compact.files.contains_key(file_name) {
      // File removed in compact
      for def_name in detailed_file.defs.keys() {
        changes.push(SnapshotChange {
          path: ChangePath::FunctionDefinition {
            file_name: file_name.clone(),
            def_name: def_name.clone(),
          },
          change_type: ChangeType::Removed,
          new_entry: None,
        });
      }
    }
  }

  changes
}

fn compare_file_definitions(
  file_name: &str,
  compact_file: &FileInSnapShot,
  detailed_file: &DetailedFileInSnapshot,
  changes: &mut Vec<SnapshotChange>,
) {
  // Compare namespace
  let compact_ns_cirru: Cirru = compact_file.ns.code.clone();
  let detailed_ns_cirru: Cirru = detailed_file.ns.code.clone().into();

  let code_changed = compact_ns_cirru != detailed_ns_cirru;
  let doc_changed = compact_file.ns.doc != detailed_file.ns.doc;

  if code_changed || doc_changed {
    let change_type = if code_changed && doc_changed {
      ChangeType::Modified
    } else if code_changed {
      ChangeType::ModifiedCode
    } else {
      ChangeType::ModifiedDoc
    };

    changes.push(SnapshotChange {
      path: ChangePath::NamespaceDefinition {
        file_name: file_name.to_string(),
      },
      change_type,
      new_entry: Some(compact_file.ns.clone()),
    });
  }

  // Compare definitions
  for (def_name, compact_entry) in &compact_file.defs {
    match detailed_file.defs.get(def_name) {
      Some(detailed_entry) => {
        // Definition exists in both, compare content
        let compact_cirru: Cirru = compact_entry.code.clone();
        let detailed_cirru: Cirru = detailed_entry.code.clone().into();

        let code_changed = compact_cirru != detailed_cirru;
        let doc_changed = compact_entry.doc != detailed_entry.doc;

        // Check if examples changed
        let compact_examples: Vec<Cirru> = compact_entry.examples.clone();
        let detailed_examples: Vec<Cirru> = detailed_entry.examples.iter().map(|e| e.clone().into()).collect();
        let examples_changed = compact_examples != detailed_examples;

        if code_changed || doc_changed || examples_changed {
          let change_type = if code_changed && doc_changed {
            ChangeType::Modified
          } else if code_changed {
            ChangeType::ModifiedCode
          } else {
            ChangeType::ModifiedDoc
          };

          // TODO better detect structural changes of branches and leaves in the future

          changes.push(SnapshotChange {
            path: ChangePath::FunctionDefinition {
              file_name: file_name.to_string(),
              def_name: def_name.clone(),
            },
            change_type,
            new_entry: Some(compact_entry.clone()),
          });
        }
      }
      None => {
        // Definition added in compact
        changes.push(SnapshotChange {
          path: ChangePath::FunctionDefinition {
            file_name: file_name.to_string(),
            def_name: def_name.clone(),
          },
          change_type: ChangeType::Added,
          new_entry: Some(compact_entry.clone()),
        });
      }
    }
  }

  // Check for removed definitions
  for def_name in detailed_file.defs.keys() {
    if !compact_file.defs.contains_key(def_name) {
      changes.push(SnapshotChange {
        path: ChangePath::FunctionDefinition {
          file_name: file_name.to_string(),
          def_name: def_name.clone(),
        },
        change_type: ChangeType::Removed,
        new_entry: None,
      });
    }
  }
}

fn apply_snapshot_changes(detailed: &mut DetailedSnapshot, changes: &[SnapshotChange]) {
  for change in changes {
    match change.change_type {
      ChangeType::Added => {
        if let Some(new_entry) = &change.new_entry {
          apply_add_change(detailed, &change.path, new_entry);
        }
      }
      ChangeType::Modified | ChangeType::ModifiedCode | ChangeType::ModifiedDoc => {
        if let Some(new_entry) = &change.new_entry {
          apply_modify_change(detailed, &change.path, new_entry, &change.change_type);
        }
      }
      ChangeType::Removed => {
        apply_remove_change(detailed, &change.path);
      }
    }
  }
}

fn apply_add_change(detailed: &mut DetailedSnapshot, path: &ChangePath, new_entry: &CodeEntry) {
  match path {
    ChangePath::FunctionDefinition { file_name, def_name } => {
      // Create file if it doesn't exist
      if !detailed.files.contains_key(file_name) {
        use calcit::detailed_snapshot::{DetailedCodeEntry, DetailedFileInSnapshot};
        use std::collections::HashMap;

        // Create empty namespace entry
        let empty_ns = DetailedCodeEntry {
          doc: String::new(),
          examples: vec![],
          code: cirru_parser::Cirru::Leaf("".into()).into(),
        };

        detailed.files.insert(
          file_name.clone(),
          DetailedFileInSnapshot {
            ns: empty_ns,
            defs: HashMap::new(),
          },
        );
      }

      if let Some(file) = detailed.files.get_mut(file_name) {
        file.defs.insert(def_name.clone(), new_entry.clone().into());
      }
    }
    ChangePath::NamespaceDefinition { file_name } => {
      // Create file if it doesn't exist
      if !detailed.files.contains_key(file_name) {
        use calcit::detailed_snapshot::DetailedFileInSnapshot;
        use std::collections::HashMap;

        detailed.files.insert(
          file_name.clone(),
          DetailedFileInSnapshot {
            ns: new_entry.clone().into(),
            defs: HashMap::new(),
          },
        );
      } else if let Some(file) = detailed.files.get_mut(file_name) {
        file.ns = new_entry.clone().into();
      }
    }
  }
}

fn apply_modify_change(detailed: &mut DetailedSnapshot, path: &ChangePath, new_entry: &CodeEntry, change_type: &ChangeType) {
  match path {
    ChangePath::FunctionDefinition { file_name, def_name } => {
      if let Some(file) = detailed.files.get_mut(file_name) {
        if let Some(existing_def) = file.defs.get_mut(def_name) {
          // Update document part
          existing_def.doc = new_entry.doc.clone();

          // Update examples
          existing_def.examples = new_entry.examples.iter().map(|e| e.clone().into()).collect();

          // If not only document changes, also update code part
          if *change_type != ChangeType::ModifiedDoc {
            existing_def.code = new_entry.code.clone().into();
          }
        }
      }
    }
    ChangePath::NamespaceDefinition { file_name } => {
      if let Some(file) = detailed.files.get_mut(file_name) {
        // Update document part
        file.ns.doc = new_entry.doc.clone();

        // If not only document changes, also update code part
        if *change_type != ChangeType::ModifiedDoc {
          file.ns.code = new_entry.code.clone().into();
        }
      }
    }
  }
}

fn apply_remove_change(detailed: &mut DetailedSnapshot, path: &ChangePath) {
  match path {
    ChangePath::FunctionDefinition { file_name, def_name } => {
      if let Some(file) = detailed.files.get_mut(file_name) {
        file.defs.remove(def_name);
      }
    }
    ChangePath::NamespaceDefinition { .. } => {
      // Namespace removal not supported
    }
  }
}
