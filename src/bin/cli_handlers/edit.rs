//! Edit subcommand handlers
//!
//! Handles: cr edit - code editing operations (definitions, namespaces, modules, configs)
//!
//! Supports code input via:
//! - `--file <path>` - read from file
//! - `--json <string>` - inline JSON string
//! - `--stdin` - read from stdin

use super::query::cirru_to_json_with_depth;
use calcit::cli_args::{
  EditAddModuleCommand, EditAddNsCommand, EditCommand, EditDeleteDefCommand, EditDeleteModuleCommand, EditDeleteNsCommand,
  EditOperateAtCommand, EditSetConfigCommand, EditSubcommand, EditUpdateDefDocCommand, EditUpdateImportsCommand,
  EditUpdateNsDocCommand, EditUpsertDefCommand,
};
use calcit::snapshot::{self, CodeEntry, FileInSnapShot, Snapshot, save_snapshot_to_file};
use cirru_parser::Cirru;
use colored::Colorize;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::sync::Arc;

pub fn handle_edit_command(cmd: &EditCommand, snapshot_file: &str) -> Result<(), String> {
  match &cmd.subcommand {
    EditSubcommand::UpsertDef(opts) => handle_upsert_def(opts, snapshot_file),
    EditSubcommand::DeleteDef(opts) => handle_delete_def(opts, snapshot_file),
    EditSubcommand::UpdateDefDoc(opts) => handle_update_def_doc(opts, snapshot_file),
    EditSubcommand::OperateAt(opts) => handle_operate_at(opts, snapshot_file),
    EditSubcommand::AddNs(opts) => handle_add_ns(opts, snapshot_file),
    EditSubcommand::DeleteNs(opts) => handle_delete_ns(opts, snapshot_file),
    EditSubcommand::UpdateImports(opts) => handle_update_imports(opts, snapshot_file),
    EditSubcommand::UpdateNsDoc(opts) => handle_update_ns_doc(opts, snapshot_file),
    EditSubcommand::AddModule(opts) => handle_add_module(opts, snapshot_file),
    EditSubcommand::DeleteModule(opts) => handle_delete_module(opts, snapshot_file),
    EditSubcommand::SetConfig(opts) => handle_set_config(opts, snapshot_file),
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Utility functions
// ═══════════════════════════════════════════════════════════════════════════════

fn load_snapshot(snapshot_file: &str) -> Result<Snapshot, String> {
  let content = fs::read_to_string(snapshot_file).map_err(|e| format!("Failed to read {snapshot_file}: {e}"))?;

  let edn = cirru_edn::parse(&content).map_err(|e| format!("Failed to parse EDN: {e}"))?;

  snapshot::load_snapshot_data(&edn, snapshot_file).map_err(|e| format!("Failed to load snapshot: {e}"))
}

fn save_snapshot(snapshot: &Snapshot, snapshot_file: &str) -> Result<(), String> {
  save_snapshot_to_file(snapshot_file, snapshot)
}

/// Read code input from file, json option, or stdin
fn read_code_input(file: &Option<String>, json: &Option<String>, stdin: bool) -> Result<Option<String>, String> {
  if stdin {
    let mut buffer = String::new();
    io::stdin()
      .read_to_string(&mut buffer)
      .map_err(|e| format!("Failed to read from stdin: {e}"))?;
    Ok(Some(buffer.trim().to_string()))
  } else if let Some(path) = file {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file '{path}': {e}"))?;
    Ok(Some(content.trim().to_string()))
  } else if let Some(j) = json {
    Ok(Some(j.clone()))
  } else {
    Ok(None)
  }
}

/// Parse JSON string to Cirru syntax tree
fn json_to_cirru(json_str: &str) -> Result<Cirru, String> {
  let json: serde_json::Value = serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {e}"))?;

  json_value_to_cirru(&json)
}

fn json_value_to_cirru(json: &serde_json::Value) -> Result<Cirru, String> {
  match json {
    serde_json::Value::String(s) => Ok(Cirru::Leaf(Arc::from(s.as_str()))),
    serde_json::Value::Array(arr) => {
      let items: Result<Vec<Cirru>, String> = arr.iter().map(json_value_to_cirru).collect();
      Ok(Cirru::List(items?))
    }
    serde_json::Value::Number(n) => Ok(Cirru::Leaf(Arc::from(n.to_string()))),
    serde_json::Value::Bool(b) => Ok(Cirru::Leaf(Arc::from(b.to_string()))),
    serde_json::Value::Null => Ok(Cirru::Leaf(Arc::from("nil"))),
    serde_json::Value::Object(_) => Err("JSON objects cannot be converted to Cirru syntax tree".to_string()),
  }
}

/// Parse path string like "2,1,0" to Vec<usize>
fn parse_path(path_str: &str) -> Result<Vec<usize>, String> {
  if path_str.is_empty() {
    return Ok(vec![]);
  }

  path_str
    .split(',')
    .map(|s| s.trim().parse::<usize>().map_err(|e| format!("Invalid path index '{s}': {e}")))
    .collect()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Definition operations
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_upsert_def(opts: &EditUpsertDefCommand, snapshot_file: &str) -> Result<(), String> {
  let json_str = read_code_input(&opts.file, &opts.json, opts.stdin)?.ok_or("Code input required: use --file, --json, or --stdin")?;

  let syntax_tree = json_to_cirru(&json_str)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace exists
  let file_data = snapshot
    .files
    .get_mut(&opts.namespace)
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  // Check if definition exists
  let exists = file_data.defs.contains_key(&opts.definition);

  if exists && !opts.replace {
    return Err(format!(
      "Definition '{}' already exists in namespace '{}'. Use --replace to overwrite.",
      opts.definition, opts.namespace
    ));
  }

  // Create or update definition
  let code_entry = CodeEntry::from_code(syntax_tree);
  file_data.defs.insert(opts.definition.clone(), code_entry);

  save_snapshot(&snapshot, snapshot_file)?;

  if exists {
    println!(
      "{} Updated definition '{}' in namespace '{}'",
      "✓".green(),
      opts.definition.cyan(),
      opts.namespace
    );
  } else {
    println!(
      "{} Created definition '{}' in namespace '{}'",
      "✓".green(),
      opts.definition.cyan(),
      opts.namespace
    );
  }

  Ok(())
}

fn handle_delete_def(opts: &EditDeleteDefCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  let file_data = snapshot
    .files
    .get_mut(&opts.namespace)
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  if file_data.defs.remove(&opts.definition).is_none() {
    return Err(format!(
      "Definition '{}' not found in namespace '{}'",
      opts.definition, opts.namespace
    ));
  }

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Deleted definition '{}' from namespace '{}'",
    "✓".green(),
    opts.definition.cyan(),
    opts.namespace
  );

  Ok(())
}

fn handle_update_def_doc(opts: &EditUpdateDefDocCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  let file_data = snapshot
    .files
    .get_mut(&opts.namespace)
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  let code_entry = file_data
    .defs
    .get_mut(&opts.definition)
    .ok_or_else(|| format!("Definition '{}' not found in namespace '{}'", opts.definition, opts.namespace))?;

  code_entry.doc = opts.doc.clone();

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Updated documentation for '{}' in namespace '{}'",
    "✓".green(),
    opts.definition.cyan(),
    opts.namespace
  );

  Ok(())
}

fn handle_operate_at(opts: &EditOperateAtCommand, snapshot_file: &str) -> Result<(), String> {
  let path = parse_path(&opts.path)?;

  // For delete operation, code input is not required
  let code_input = read_code_input(&opts.file, &opts.json, opts.stdin)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  let file_data = snapshot
    .files
    .get_mut(&opts.namespace)
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  let code_entry = file_data
    .defs
    .get_mut(&opts.definition)
    .ok_or_else(|| format!("Definition '{}' not found", opts.definition))?;

  // Apply operation at path
  let new_code = apply_operation_at_path(&code_entry.code, &path, &opts.operation, code_input.as_deref())?;

  code_entry.code = new_code.clone();

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Applied '{}' at path [{}] in '{}/{}'",
    "✓".green(),
    opts.operation.yellow(),
    opts.path,
    opts.namespace,
    opts.definition.cyan()
  );

  // Show preview of result with depth limit
  if opts.depth > 0 || opts.operation != "delete" {
    // Navigate to the affected area for preview
    let preview_target = if opts.operation == "delete" {
      // For delete, show parent
      if path.is_empty() {
        new_code.clone()
      } else {
        let parent_path = &path[..path.len() - 1];
        navigate_to_path(&new_code, parent_path).unwrap_or_else(|_| new_code.clone())
      }
    } else {
      // For other operations, show the modified node
      navigate_to_path(&new_code, &path).unwrap_or_else(|_| new_code.clone())
    };

    let depth = if opts.depth == 0 { 2 } else { opts.depth };
    let json = cirru_to_json_with_depth(&preview_target, depth, 0);
    println!("\n{}", "Preview:".bold());
    println!("{}", serde_json::to_string_pretty(&json).unwrap());
    println!("{}", format!("(depth limited to {depth})").dimmed());
  }

  Ok(())
}

fn apply_operation_at_path(code: &Cirru, path: &[usize], operation: &str, new_code_json: Option<&str>) -> Result<Cirru, String> {
  if path.is_empty() {
    // Operating on root
    return match operation {
      "replace" => {
        let json_str = new_code_json.ok_or("Code input required for replace operation")?;
        json_to_cirru(json_str)
      }
      "delete" => Err("Cannot delete root node".to_string()),
      _ => Err(format!("Operation '{operation}' not supported at root level")),
    };
  }

  // Navigate to parent and operate on child
  apply_operation_recursive(code, path, 0, operation, new_code_json)
}

fn apply_operation_recursive(
  code: &Cirru,
  path: &[usize],
  depth: usize,
  operation: &str,
  new_code_json: Option<&str>,
) -> Result<Cirru, String> {
  match code {
    Cirru::Leaf(_) => Err(format!("Cannot navigate into leaf node at depth {depth}")),
    Cirru::List(items) => {
      let idx = path[depth];
      if idx >= items.len() {
        return Err(format!("Path index {} out of bounds (list has {} items)", idx, items.len()));
      }

      if depth == path.len() - 1 {
        // At target position, apply operation
        let mut new_items = items.clone();

        match operation {
          "delete" => {
            new_items.remove(idx);
          }
          "replace" => {
            let json_str = new_code_json.ok_or("Code input required for replace operation")?;
            let new_node = json_to_cirru(json_str)?;
            new_items[idx] = new_node;
          }
          "insert-before" => {
            let json_str = new_code_json.ok_or("Code input required for insert-before operation")?;
            let new_node = json_to_cirru(json_str)?;
            new_items.insert(idx, new_node);
          }
          "insert-after" => {
            let json_str = new_code_json.ok_or("Code input required for insert-after operation")?;
            let new_node = json_to_cirru(json_str)?;
            new_items.insert(idx + 1, new_node);
          }
          "insert-child" => {
            // Insert as first child of the node at idx
            let json_str = new_code_json.ok_or("Code input required for insert-child operation")?;
            let new_node = json_to_cirru(json_str)?;
            match &new_items[idx] {
              Cirru::List(children) => {
                let mut new_children = vec![new_node];
                new_children.extend(children.clone());
                new_items[idx] = Cirru::List(new_children);
              }
              Cirru::Leaf(_) => {
                return Err("Cannot insert child into leaf node".to_string());
              }
            }
          }
          _ => {
            return Err(format!("Unknown operation: {operation}"));
          }
        }

        Ok(Cirru::List(new_items))
      } else {
        // Continue navigating
        let mut new_items = items.clone();
        new_items[idx] = apply_operation_recursive(&items[idx], path, depth + 1, operation, new_code_json)?;
        Ok(Cirru::List(new_items))
      }
    }
  }
}

fn navigate_to_path(code: &Cirru, path: &[usize]) -> Result<Cirru, String> {
  if path.is_empty() {
    return Ok(code.clone());
  }

  let mut current = code;
  for (depth, &idx) in path.iter().enumerate() {
    match current {
      Cirru::Leaf(_) => {
        return Err(format!("Cannot navigate into leaf node at depth {depth}"));
      }
      Cirru::List(items) => {
        if idx >= items.len() {
          return Err(format!(
            "Path index {} out of bounds at depth {} (list has {} items)",
            idx,
            depth,
            items.len()
          ));
        }
        current = &items[idx];
      }
    }
  }

  Ok(current.clone())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace operations
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_add_ns(opts: &EditAddNsCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  if snapshot.files.contains_key(&opts.namespace) {
    return Err(format!("Namespace '{}' already exists", opts.namespace));
  }

  // Create ns code
  let ns_code = if let Some(json_str) = read_code_input(&opts.file, &opts.json, opts.stdin)? {
    json_to_cirru(&json_str)?
  } else {
    // Default minimal ns declaration: (ns namespace-name)
    Cirru::List(vec![Cirru::Leaf(Arc::from("ns")), Cirru::Leaf(Arc::from(opts.namespace.as_str()))])
  };

  let file_entry = FileInSnapShot {
    ns: CodeEntry::from_code(ns_code),
    defs: HashMap::new(),
  };

  snapshot.files.insert(opts.namespace.clone(), file_entry);

  save_snapshot(&snapshot, snapshot_file)?;

  println!("{} Created namespace '{}'", "✓".green(), opts.namespace.cyan());

  Ok(())
}

fn handle_delete_ns(opts: &EditDeleteNsCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  if snapshot.files.remove(&opts.namespace).is_none() {
    return Err(format!("Namespace '{}' not found", opts.namespace));
  }

  save_snapshot(&snapshot, snapshot_file)?;

  println!("{} Deleted namespace '{}'", "✓".green(), opts.namespace.cyan());

  Ok(())
}

fn handle_update_imports(opts: &EditUpdateImportsCommand, snapshot_file: &str) -> Result<(), String> {
  let json_str =
    read_code_input(&opts.file, &opts.json, opts.stdin)?.ok_or("Imports input required: use --file, --json, or --stdin")?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  let file_data = snapshot
    .files
    .get_mut(&opts.namespace)
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  // Parse the imports JSON as array of import rules
  let imports_json: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| format!("Failed to parse imports JSON: {e}"))?;

  // Build new ns code with imports
  // Format: (ns namespace :require import1 import2 ...)
  let ns_name = &opts.namespace;

  let mut ns_code_items = vec![Cirru::Leaf(Arc::from("ns")), Cirru::Leaf(Arc::from(ns_name.as_str()))];

  if let serde_json::Value::Array(imports) = imports_json {
    if !imports.is_empty() {
      ns_code_items.push(Cirru::Leaf(Arc::from(":require")));
      for import in imports {
        let import_cirru = json_value_to_cirru(&import)?;
        ns_code_items.push(import_cirru);
      }
    }
  } else {
    return Err("Imports must be a JSON array".to_string());
  }

  file_data.ns.code = Cirru::List(ns_code_items);

  save_snapshot(&snapshot, snapshot_file)?;

  println!("{} Updated imports for namespace '{}'", "✓".green(), opts.namespace.cyan());

  Ok(())
}

fn handle_update_ns_doc(opts: &EditUpdateNsDocCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  let file_data = snapshot
    .files
    .get_mut(&opts.namespace)
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  file_data.ns.doc = opts.doc.clone();

  save_snapshot(&snapshot, snapshot_file)?;

  println!("{} Updated documentation for namespace '{}'", "✓".green(), opts.namespace.cyan());

  Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Module operations
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_add_module(opts: &EditAddModuleCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  if snapshot.configs.modules.contains(&opts.module_path) {
    return Err(format!("Module '{}' already exists in configs", opts.module_path));
  }

  snapshot.configs.modules.push(opts.module_path.clone());

  save_snapshot(&snapshot, snapshot_file)?;

  println!("{} Added module '{}'", "✓".green(), opts.module_path.cyan());

  Ok(())
}

fn handle_delete_module(opts: &EditDeleteModuleCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  let original_len = snapshot.configs.modules.len();
  snapshot.configs.modules.retain(|m| m != &opts.module_path);

  if snapshot.configs.modules.len() == original_len {
    return Err(format!("Module '{}' not found in configs", opts.module_path));
  }

  save_snapshot(&snapshot, snapshot_file)?;

  println!("{} Deleted module '{}'", "✓".green(), opts.module_path.cyan());

  Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Config operations
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_set_config(opts: &EditSetConfigCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  match opts.key.as_str() {
    "init-fn" | "init_fn" => {
      snapshot.configs.init_fn = opts.value.clone();
    }
    "reload-fn" | "reload_fn" => {
      snapshot.configs.reload_fn = opts.value.clone();
    }
    "version" => {
      snapshot.configs.version = opts.value.clone();
    }
    _ => {
      return Err(format!(
        "Unknown config key '{}'. Valid keys: init-fn, reload-fn, version",
        opts.key
      ));
    }
  }

  save_snapshot(&snapshot, snapshot_file)?;

  println!("{} Set config '{}' = '{}'", "✓".green(), opts.key.cyan(), opts.value);

  Ok(())
}
