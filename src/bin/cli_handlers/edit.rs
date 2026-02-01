//! Edit and Tree subcommand handlers and shared utilities
//!
//! Handles: cr edit - code editing operations (definitions, namespaces, modules, configs)
//! Shared by: cr tree - fine-grained tree operations (replace, insert, delete, swap, wrap)
//!
//! Supports code input via:
//! - `--file <path>` - read from file
//! - `--json <string>` - inline JSON string
//! - `--stdin` - read from stdin

use calcit::cli_args::{
  EditAddExampleCommand, EditAddImportCommand, EditAddModuleCommand, EditAddNsCommand, EditCommand, EditConfigCommand, EditDefCommand,
  EditDocCommand, EditExamplesCommand, EditImportsCommand, EditIncCommand, EditMvDefCommand, EditNsDocCommand, EditRmDefCommand,
  EditRmExampleCommand, EditRmImportCommand, EditRmModuleCommand, EditRmNsCommand, EditSubcommand,
};
use calcit::snapshot::{self, ChangesDict, CodeEntry, FileChangeInfo, FileInSnapShot, Snapshot, save_snapshot_to_file};
use cirru_parser::Cirru;
use colored::Colorize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::Arc;

use super::common::{ERR_CODE_INPUT_REQUIRED, json_value_to_cirru, parse_input_to_cirru, read_code_input};
use super::tips::Tips;

/// Parse "namespace/definition" format into (namespace, definition)
pub(crate) fn parse_target(target: &str) -> Result<(&str, &str), String> {
  target
    .rsplit_once('/')
    .ok_or_else(|| format!("Invalid target format: '{target}'. Expected 'namespace/definition' (e.g. 'app.core/main')"))
}

/// Process a node by replacing placeholders with references to original node or its branches
pub(crate) fn process_node_with_references(
  node: &Cirru,
  original_node: Option<&Cirru>,
  refer_original: &Option<String>,
  refer_inner_branch: &Option<String>,
  refer_inner_placeholder: &Option<String>,
) -> Result<Cirru, String> {
  let original = original_node.ok_or("Original node required for reference replacement")?;

  // Parse inner branch if provided
  let inner_branch_info: Option<(String, Vec<usize>)> = match (refer_inner_branch, refer_inner_placeholder) {
    (Some(path_str), Some(placeholder)) => {
      let path: Vec<usize> = path_str
        .split(',')
        .map(|s| s.trim().parse::<usize>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Invalid inner branch path '{path_str}': {e}"))?;
      Some((placeholder.clone(), path))
    }
    (Some(_), None) => {
      return Err("--refer-inner-branch requires --refer-inner-placeholder".to_string());
    }
    (None, Some(_)) => {
      return Err("--refer-inner-placeholder requires --refer-inner-branch".to_string());
    }
    (None, None) => None,
  };

  match node {
    Cirru::Leaf(s) => {
      // Check if this leaf matches the refer_original placeholder
      if let Some(placeholder) = refer_original {
        if s.as_ref() == placeholder {
          return Ok(original.clone());
        }
      }
      // Check if this leaf matches the refer_inner_branch placeholder
      if let Some((placeholder, path)) = &inner_branch_info {
        if s.as_ref() == placeholder {
          return navigate_to_path(original, path);
        }
      }
      Ok(node.clone())
    }
    Cirru::List(items) => {
      let processed_items: Result<Vec<Cirru>, String> = items
        .iter()
        .map(|item| process_node_with_references(item, Some(original), refer_original, refer_inner_branch, refer_inner_placeholder))
        .collect();
      Ok(Cirru::List(processed_items?))
    }
  }
}

pub fn handle_edit_command(cmd: &EditCommand, snapshot_file: &str) -> Result<(), String> {
  match &cmd.subcommand {
    EditSubcommand::Def(opts) => handle_def(opts, snapshot_file),
    EditSubcommand::Mv(opts) => handle_mv_def(opts, snapshot_file),
    EditSubcommand::RmDef(opts) => handle_rm_def(opts, snapshot_file),
    EditSubcommand::Doc(opts) => handle_doc(opts, snapshot_file),
    EditSubcommand::Examples(opts) => handle_examples(opts, snapshot_file),
    EditSubcommand::AddExample(opts) => handle_add_example(opts, snapshot_file),
    EditSubcommand::RmExample(opts) => handle_rm_example(opts, snapshot_file),
    EditSubcommand::AddNs(opts) => handle_add_ns(opts, snapshot_file),
    EditSubcommand::RmNs(opts) => handle_rm_ns(opts, snapshot_file),
    EditSubcommand::Imports(opts) => handle_imports(opts, snapshot_file),
    EditSubcommand::AddImport(opts) => handle_add_import(opts, snapshot_file),
    EditSubcommand::RmImport(opts) => handle_rm_import(opts, snapshot_file),
    EditSubcommand::NsDoc(opts) => handle_ns_doc(opts, snapshot_file),
    EditSubcommand::AddModule(opts) => handle_add_module(opts, snapshot_file),
    EditSubcommand::RmModule(opts) => handle_rm_module(opts, snapshot_file),
    EditSubcommand::Config(opts) => handle_config(opts, snapshot_file),
    EditSubcommand::Inc(opts) => handle_inc(opts, snapshot_file),
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Utility functions
// ═══════════════════════════════════════════════════════════════════════════════

pub(crate) fn load_snapshot(snapshot_file: &str) -> Result<Snapshot, String> {
  let content = fs::read_to_string(snapshot_file).map_err(|e| format!("Failed to read {snapshot_file}: {e}"))?;

  let edn = cirru_edn::parse(&content).map_err(|e| format!("Failed to parse EDN: {e}"))?;

  snapshot::load_snapshot_data(&edn, snapshot_file).map_err(|e| format!("Failed to load snapshot: {e}"))
}

pub(crate) fn save_snapshot(snapshot: &Snapshot, snapshot_file: &str) -> Result<(), String> {
  save_snapshot_to_file(snapshot_file, snapshot)
}

/// Check if namespace belongs to the current package (can be edited)
pub(crate) fn check_ns_editable(snapshot: &Snapshot, namespace: &str) -> Result<(), String> {
  let pkg = &snapshot.package;
  // Namespace must match package name or start with "package."
  if namespace == pkg || namespace.starts_with(&format!("{pkg}.")) {
    Ok(())
  } else {
    Err(format!(
      "Cannot modify namespace '{namespace}': only namespaces under package '{pkg}' can be edited.\nThis namespace belongs to a dependency or core library."
    ))
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Definition operations
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_def(opts: &EditDefCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let raw = read_code_input(&opts.file, &opts.code, &opts.json, opts.stdin)?.ok_or(ERR_CODE_INPUT_REQUIRED)?;
  let auto_json = opts.code.is_some();

  let syntax_tree = parse_input_to_cirru(&raw, &opts.json, opts.json_input, opts.leaf, auto_json)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, namespace)?;

  // Check if namespace exists
  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  // Check if definition exists
  let exists = file_data.defs.contains_key(definition);

  if exists {
    return Err(format!(
      "Definition '{definition}' already exists in namespace '{namespace}'.\n\
       To replace the entire definition, use: cr tree replace {namespace}/{definition} -p '' -e '<code>'\n\
       To modify parts of the definition, use: cr tree replace {namespace}/{definition} -p '<path>' -e '<code>'"
    ));
  }

  // Create definition
  let code_entry = CodeEntry::from_code(syntax_tree);
  file_data.defs.insert(definition.to_string(), code_entry);

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Created definition '{}' in namespace '{}'",
    "✓".green(),
    definition.cyan(),
    namespace
  );
  println!();
  println!("{}", "Next steps:".blue().bold());
  println!("  • View definition: {} '{}/{}'", "cr query def".cyan(), namespace, definition);
  println!("  • Find usages: {} '{}/{}'", "cr query usages".cyan(), namespace, definition);
  println!(
    "  • Add to imports: {} <target-ns> '{}' --refer '{}'",
    "cr edit add-import".cyan(),
    namespace,
    definition
  );
  println!();
  let mut tips = Tips::new();
  tips.add(format!(
    "Use single quotes around '{namespace}/{definition}' to avoid shell escaping issues."
  ));
  tips.add(format!("Example: cr tree show '{namespace}/{definition}'"));
  tips.print();
  Ok(())
}

fn handle_rm_def(opts: &EditRmDefCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  if file_data.defs.remove(definition).is_none() {
    return Err(format!("Definition '{definition}' not found in namespace '{namespace}'"));
  }

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Deleted definition '{}' from namespace '{}'",
    "✓".green(),
    definition.cyan(),
    namespace
  );

  Ok(())
}

fn handle_mv_def(opts: &EditMvDefCommand, snapshot_file: &str) -> Result<(), String> {
  let (source_ns, source_def) = parse_target(&opts.source)?;
  let (target_ns, target_def) = parse_target(&opts.target)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  check_ns_editable(&snapshot, source_ns)?;
  check_ns_editable(&snapshot, target_ns)?;

  if source_ns == target_ns && source_def == target_def {
    return Err("Source and target are identical; nothing to move.".to_string());
  }

  if source_ns == target_ns {
    let file_data = snapshot
      .files
      .get_mut(source_ns)
      .ok_or_else(|| format!("Namespace '{source_ns}' not found"))?;

    if !file_data.defs.contains_key(source_def) {
      return Err(format!("Definition '{source_def}' not found in namespace '{source_ns}'"));
    }
    if file_data.defs.contains_key(target_def) {
      return Err(format!("Definition '{target_def}' already exists in namespace '{source_ns}'"));
    }

    let entry = file_data.defs.remove(source_def).expect("checked definition exists");
    file_data.defs.insert(target_def.to_string(), entry);
  } else {
    let source_exists = snapshot
      .files
      .get(source_ns)
      .ok_or_else(|| format!("Namespace '{source_ns}' not found"))?
      .defs
      .contains_key(source_def);
    if !source_exists {
      return Err(format!("Definition '{source_def}' not found in namespace '{source_ns}'"));
    }

    let target_exists = snapshot
      .files
      .get(target_ns)
      .ok_or_else(|| format!("Namespace '{target_ns}' not found"))?
      .defs
      .contains_key(target_def);
    if target_exists {
      return Err(format!("Definition '{target_def}' already exists in namespace '{target_ns}'"));
    }

    let entry = {
      let source_file = snapshot
        .files
        .get_mut(source_ns)
        .ok_or_else(|| format!("Namespace '{source_ns}' not found"))?;
      source_file.defs.remove(source_def).expect("checked definition exists")
    };

    let target_file = snapshot
      .files
      .get_mut(target_ns)
      .ok_or_else(|| format!("Namespace '{target_ns}' not found"))?;
    target_file.defs.insert(target_def.to_string(), entry);
  }

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Moved definition '{}' from '{}' to '{}'",
    "✓".green(),
    source_def.cyan(),
    source_ns.cyan(),
    format!("{target_ns}/{target_def}").cyan()
  );

  Ok(())
}

fn handle_doc(opts: &EditDocCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get_mut(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found in namespace '{namespace}'"))?;

  code_entry.doc = opts.doc.clone();

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Updated documentation for '{}' in namespace '{}'",
    "✓".green(),
    definition.cyan(),
    namespace
  );

  Ok(())
}

fn handle_examples(opts: &EditExamplesCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get_mut(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found in namespace '{namespace}'"))?;

  // Handle --clear flag
  if opts.clear {
    let old_count = code_entry.examples.len();
    code_entry.examples.clear();
    save_snapshot(&snapshot, snapshot_file)?;
    println!(
      "{} Cleared {} example(s) for '{}' in namespace '{}'",
      "✓".green(),
      old_count,
      definition.cyan(),
      namespace
    );
    return Ok(());
  }

  // Read examples input
  let code_input = read_code_input(&opts.file, &opts.code, &opts.json, opts.stdin)?;
  let raw = code_input
    .as_deref()
    .ok_or("Examples input required: use --file, --code, --json, --stdin, or --clear")?;

  // Parse examples - expect an array of Cirru expressions
  let examples: Vec<Cirru> = if opts.leaf {
    vec![Cirru::Leaf(Arc::from(raw))]
  } else if opts.json.is_some() || opts.json_input {
    // Parse as JSON array
    let json_value: serde_json::Value = serde_json::from_str(raw).map_err(|e| format!("Failed to parse JSON: {e}"))?;
    match json_value {
      serde_json::Value::Array(arr) => arr.iter().map(json_value_to_cirru).collect::<Result<Vec<_>, _>>()?,
      _ => return Err("Expected JSON array of examples".to_string()),
    }
  } else {
    // Parse as Cirru text - each top-level expression is an example
    cirru_parser::parse(raw).map_err(|e| format!("Failed to parse Cirru: {e}"))?
  };

  let count = examples.len();
  code_entry.examples = examples;

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Set {} example(s) for '{}' in namespace '{}'",
    "✓".green(),
    count,
    definition.cyan(),
    namespace
  );

  Ok(())
}

fn handle_add_example(opts: &EditAddExampleCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get_mut(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found in namespace '{namespace}'"))?;

  // Read example input
  let code_input = read_code_input(&opts.file, &opts.code, &opts.json, opts.stdin)?;
  let raw = code_input
    .as_deref()
    .ok_or("Example input required: use --file, --code, --json, or --stdin")?;

  // Parse example
  let example: Cirru = parse_input_to_cirru(raw, &opts.json, opts.json_input, opts.leaf, opts.code.is_some())?;

  // Insert at specified position or append
  let position = opts.at.unwrap_or(code_entry.examples.len());
  if position > code_entry.examples.len() {
    return Err(format!("Position {} out of range (max: {})", position, code_entry.examples.len()));
  }

  code_entry.examples.insert(position, example);

  let total_count = code_entry.examples.len();

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Added example at position {} for '{}' in namespace '{}' (total: {})",
    "✓".green(),
    position,
    definition.cyan(),
    namespace,
    total_count
  );

  Ok(())
}

fn handle_rm_example(opts: &EditRmExampleCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get_mut(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found in namespace '{namespace}'"))?;

  // Validate index
  if opts.index >= code_entry.examples.len() {
    return Err(format!(
      "Index {} out of range (max: {})",
      opts.index,
      code_entry.examples.len().saturating_sub(1)
    ));
  }

  // Remove example
  code_entry.examples.remove(opts.index);

  let remaining_count = code_entry.examples.len();

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Removed example at index {} from '{}' in namespace '{}' (remaining: {})",
    "✓".green(),
    opts.index,
    definition.cyan(),
    namespace,
    remaining_count
  );

  Ok(())
}

pub(crate) fn apply_operation_at_path(
  code: &Cirru,
  path: &[usize],
  operation: &str,
  new_node: Option<&Cirru>,
) -> Result<Cirru, String> {
  if path.is_empty() {
    // Operating on root
    return match operation {
      "replace" => {
        let node = new_node.ok_or("Code input required for replace operation")?;
        Ok(node.clone())
      }
      "delete" => Err("Cannot delete root node".to_string()),
      _ => Err(format!("Operation '{operation}' not supported at root level")),
    };
  }

  // Navigate to parent and operate on child
  apply_operation_recursive(code, path, 0, operation, new_node)
}

fn apply_operation_recursive(
  code: &Cirru,
  path: &[usize],
  depth: usize,
  operation: &str,
  new_node: Option<&Cirru>,
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
            let newn = new_node.ok_or("Code input required for replace operation")?;
            new_items[idx] = newn.clone();
          }
          "insert-before" => {
            let newn = new_node.ok_or("Code input required for insert-before operation")?;
            new_items.insert(idx, newn.clone());
          }
          "insert-after" => {
            let newn = new_node.ok_or("Code input required for insert-after operation")?;
            new_items.insert(idx + 1, newn.clone());
          }
          "insert-child" => {
            // Insert as first child of the node at idx
            let newn = new_node.ok_or("Code input required for insert-child operation")?;
            match &new_items[idx] {
              Cirru::List(children) => {
                let mut new_children = vec![newn.clone()];
                new_children.extend(children.clone());
                new_items[idx] = Cirru::List(new_children);
              }
              Cirru::Leaf(_) => {
                return Err("Cannot insert child into leaf node".to_string());
              }
            }
          }
          "append-child" => {
            // Insert as last child of the node at idx
            let newn = new_node.ok_or("Code input required for append-child operation")?;
            match &new_items[idx] {
              Cirru::List(children) => {
                let mut new_children = children.clone();
                new_children.push(newn.clone());
                new_items[idx] = Cirru::List(new_children);
              }
              Cirru::Leaf(_) => {
                return Err("Cannot append child to leaf node".to_string());
              }
            }
          }
          "swap-next-sibling" => {
            // Swap current node with next sibling
            if idx + 1 >= new_items.len() {
              return Err(format!("Cannot swap: no next sibling at index {idx}"));
            }
            new_items.swap(idx, idx + 1);
          }
          "swap-prev-sibling" => {
            // Swap current node with previous sibling
            if idx == 0 {
              return Err("Cannot swap: no previous sibling at index 0".to_string());
            }
            new_items.swap(idx - 1, idx);
          }
          _ => {
            return Err(format!("Unknown operation: {operation}"));
          }
        }

        Ok(Cirru::List(new_items))
      } else {
        // Continue navigating
        let mut new_items = items.clone();
        new_items[idx] = apply_operation_recursive(&items[idx], path, depth + 1, operation, new_node)?;
        Ok(Cirru::List(new_items))
      }
    }
  }
}

pub(crate) fn navigate_to_path(code: &Cirru, path: &[usize]) -> Result<Cirru, String> {
  if path.is_empty() {
    return Ok(code.clone());
  }

  let mut current = code;
  for (depth, &idx) in path.iter().enumerate() {
    match current {
      Cirru::Leaf(_) => {
        let partial_path = path[..depth].iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        return Err(format!(
          "Cannot navigate into leaf node at depth {depth}\n   Valid path stops at: [{partial_path}]\n   Tip: Use 'cr tree show' to explore the tree structure"
        ));
      }
      Cirru::List(items) => {
        if idx >= items.len() {
          let partial_path = path[..depth].iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
          let attempted_path = path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
          return Err(format!(
            "Path index {} out of bounds at depth {} (list has {} items)\n   Attempted path: [{}]\n   Valid path: [{}]\n   Valid index range at this level: 0-{}\n   Tip: Use 'cr tree show' with parent path to see available indices",
            idx,
            depth,
            items.len(),
            attempted_path,
            partial_path,
            items.len().saturating_sub(1)
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

  // Check if namespace can be edited (must be under current package)
  check_ns_editable(&snapshot, &opts.namespace)?;

  if snapshot.files.contains_key(&opts.namespace) {
    return Err(format!("Namespace '{}' already exists", opts.namespace));
  }

  // Create ns code
  let auto_json = opts.code.is_some();

  let ns_code = if let Some(raw) = read_code_input(&opts.file, &opts.code, &opts.json, opts.stdin)? {
    parse_input_to_cirru(&raw, &opts.json, opts.json_input, opts.leaf, auto_json)?
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

fn handle_rm_ns(opts: &EditRmNsCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, &opts.namespace)?;

  if snapshot.files.remove(&opts.namespace).is_none() {
    return Err(format!("Namespace '{}' not found", opts.namespace));
  }

  save_snapshot(&snapshot, snapshot_file)?;

  println!("{} Deleted namespace '{}'", "✓".green(), opts.namespace.cyan());

  Ok(())
}

fn handle_imports(opts: &EditImportsCommand, snapshot_file: &str) -> Result<(), String> {
  let raw = read_code_input(&opts.file, &opts.code, &opts.json, opts.stdin)?
    .ok_or("Imports input required: use --file, --code, --json, or --stdin")?;
  let auto_json = opts.code.is_some();

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, &opts.namespace)?;

  let file_data = snapshot
    .files
    .get_mut(&opts.namespace)
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  // Determine input format: JSON (if requested) or Cirru (default)
  let imports_json: serde_json::Value = if opts.json.is_some() || opts.json_input {
    serde_json::from_str(&raw)
      .map_err(|e| format!("Failed to parse imports JSON: {e}. If you meant Cirru input, omit --json-input or pass --cirru."))?
  } else {
    // Parse as cirru and convert to JSON value
    let cirru_node = parse_input_to_cirru(&raw, &opts.json, opts.json_input, opts.leaf, auto_json)?;
    use super::common::cirru_to_json_value;
    cirru_to_json_value(&cirru_node)
  };

  // Build new ns code with imports
  // Format: (ns namespace :require import1 import2 ...)
  let ns_name = &opts.namespace;

  let mut ns_code_items = vec![Cirru::Leaf(Arc::from("ns")), Cirru::Leaf(Arc::from(ns_name.as_str()))];

  if let serde_json::Value::Array(imports) = imports_json {
    if !imports.is_empty() {
      ns_code_items.push(Cirru::Leaf(Arc::from(":require")));
      for import in imports {
        use super::common::json_value_to_cirru;
        let import_cirru = json_value_to_cirru(&import)?;
        ns_code_items.push(import_cirru);
      }
    }
  } else {
    return Err("Imports must be a JSON/Cirru array (e.g. [(require ...)]).".to_string());
  }

  // Extract old imports for comparison
  let old_imports = extract_require_list(&file_data.ns.code);
  let _old_import_rules = extract_require_rules(&file_data.ns.code);

  file_data.ns.code = Cirru::List(ns_code_items);

  // Extract new imports
  let new_imports = extract_require_list(&file_data.ns.code);
  let _new_import_rules = extract_require_rules(&file_data.ns.code);

  save_snapshot(&snapshot, snapshot_file)?;

  // Show what changed
  println!("{} Updated imports for namespace '{}'", "✓".green(), opts.namespace.cyan());

  // Show removed imports
  let removed: Vec<_> = old_imports.iter().filter(|old| !new_imports.contains(old)).collect();
  if !removed.is_empty() {
    println!("  {} Removed:", "-".red());
    for import in removed {
      println!("    {}", import.dimmed());
    }
  }

  // Show added imports
  let added: Vec<_> = new_imports.iter().filter(|new| !old_imports.contains(new)).collect();
  let mut added_namespaces = Vec::new();
  if !added.is_empty() {
    println!("  {} Added:", "+".green());
    for import in &added {
      println!("    {import}");
      // Extract namespace from import (first token before :refer or :as)
      if let Some(first_token) = import.split_whitespace().next() {
        if first_token.starts_with('(') {
          if let Some(ns) = import.split_whitespace().next().and_then(|s| s.strip_prefix('(')) {
            added_namespaces.push(ns.to_string());
          }
        } else {
          added_namespaces.push(first_token.to_string());
        }
      }
    }
  }

  // Show unchanged count if there are any
  let unchanged_count = old_imports.iter().filter(|old| new_imports.contains(old)).count();
  if unchanged_count > 0 {
    println!("  {} {} unchanged", "·".dimmed(), format!("{unchanged_count}").dimmed());
  }

  // Show detailed tips for newly added imports
  if !added.is_empty() {
    println!();
    println!("{}", "Usage tips for new imports:".dimmed());

    // Parse each added import string to provide tips
    for added_str in &added {
      // Parse the import string back to Cirru to analyze it
      if let Ok(parsed) = cirru_parser::parse(added_str) {
        if let Some(rule) = parsed.first() {
          if let Some(source_ns) = get_require_source_ns(rule) {
            print_import_usage_tips(rule, &source_ns);
          }
        }
      }
    }
  }

  Ok(())
}

/// Extract formatted import list from ns code for comparison
fn extract_require_list(ns_code: &Cirru) -> Vec<String> {
  let mut imports = Vec::new();

  if let Cirru::List(items) = ns_code {
    let mut in_require = false;
    for item in items {
      if let Cirru::Leaf(s) = item {
        if s.as_ref() == ":require" {
          in_require = true;
          continue;
        }
      }
      if in_require {
        // Format each import as one-liner
        if let Ok(formatted) = item.format_one_liner() {
          imports.push(formatted);
        }
      }
    }
  }

  imports
}

/// Extract the source namespace from a require rule
/// e.g. from `(calcit.core :refer ...)` extract `calcit.core`
fn get_require_source_ns(rule: &Cirru) -> Option<String> {
  match rule {
    Cirru::List(items) if !items.is_empty() => match &items[0] {
      Cirru::Leaf(s) => Some(s.to_string()),
      _ => None,
    },
    _ => None,
  }
}

/// Extract existing require rules from ns code
/// Handles structure: ["ns", "namespace", [":require", rule1, rule2, ...]]
fn extract_require_rules(ns_code: &Cirru) -> Vec<Cirru> {
  let mut rules = vec![];
  if let Cirru::List(items) = ns_code {
    for item in items.iter().skip(2) {
      // skip "ns" and namespace name
      if let Cirru::List(inner) = item {
        if let Some(Cirru::Leaf(first)) = inner.first() {
          if first.as_ref() == ":require" {
            // Found [:require rule1 rule2 ...]
            rules.extend(inner.iter().skip(1).cloned());
            break;
          }
        }
      }
    }
  }
  rules
}

/// Build ns code from namespace name and require rules
/// Produces structure: ["ns", "namespace", [":require", rule1, rule2, ...]]
fn build_ns_code(ns_name: &str, rules: &[Cirru]) -> Cirru {
  let mut items = vec![Cirru::Leaf(Arc::from("ns")), Cirru::Leaf(Arc::from(ns_name))];

  if !rules.is_empty() {
    let mut require_list = vec![Cirru::Leaf(Arc::from(":require"))];
    require_list.extend(rules.iter().cloned());
    items.push(Cirru::List(require_list));
  }

  Cirru::List(items)
}

fn handle_add_import(opts: &EditAddImportCommand, snapshot_file: &str) -> Result<(), String> {
  let raw = read_code_input(&opts.file, &opts.code, &opts.json, opts.stdin)?
    .ok_or("Import rule input required: use --file, --code, --json, or --stdin")?;

  let auto_json = opts.code.is_some();

  let new_rule = parse_input_to_cirru(&raw, &opts.json, opts.json_input, opts.leaf, auto_json)?;

  // Validate that the rule has a source namespace
  let new_source_ns =
    get_require_source_ns(&new_rule).ok_or("Invalid require rule: first element must be a namespace name (e.g. 'calcit.core')")?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, &opts.namespace)?;

  let file_data = snapshot
    .files
    .get_mut(&opts.namespace)
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  // Extract existing rules
  let mut rules = extract_require_rules(&file_data.ns.code);

  // Check if rule for this source namespace already exists
  let existing_idx = rules
    .iter()
    .position(|r| get_require_source_ns(r).as_deref() == Some(&new_source_ns));

  if let Some(idx) = existing_idx {
    if opts.overwrite {
      rules[idx] = new_rule.clone();
      println!(
        "{} Replaced require rule for '{}' in namespace '{}'",
        "✓".green(),
        new_source_ns.cyan(),
        opts.namespace
      );
    } else {
      return Err(format!(
        "Require rule for '{}' already exists in namespace '{}'. Use --overwrite to replace.",
        new_source_ns, opts.namespace
      ));
    }
  } else {
    rules.push(new_rule.clone());
    println!(
      "{} Added require rule for '{}' in namespace '{}'",
      "✓".green(),
      new_source_ns.cyan(),
      opts.namespace
    );
  }

  // Rebuild ns code
  file_data.ns.code = build_ns_code(&opts.namespace, &rules);

  save_snapshot(&snapshot, snapshot_file)?;

  // Show usage tips based on import type
  print_import_usage_tips(&new_rule, &new_source_ns);

  Ok(())
}

fn handle_rm_import(opts: &EditRmImportCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, &opts.namespace)?;

  let file_data = snapshot
    .files
    .get_mut(&opts.namespace)
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  // Extract existing rules
  let mut rules = extract_require_rules(&file_data.ns.code);

  // Find and remove the rule for the specified source namespace
  let original_len = rules.len();
  rules.retain(|r| get_require_source_ns(r).as_deref() != Some(&opts.source_ns));

  if rules.len() == original_len {
    return Err(format!(
      "No require rule found for '{}' in namespace '{}'",
      opts.source_ns, opts.namespace
    ));
  }

  // Rebuild ns code
  file_data.ns.code = build_ns_code(&opts.namespace, &rules);

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Removed require rule for '{}' from namespace '{}'",
    "✓".green(),
    opts.source_ns.cyan(),
    opts.namespace
  );

  Ok(())
}

fn handle_ns_doc(opts: &EditNsDocCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, &opts.namespace)?;

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

fn handle_rm_module(opts: &EditRmModuleCommand, snapshot_file: &str) -> Result<(), String> {
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

fn handle_config(opts: &EditConfigCommand, snapshot_file: &str) -> Result<(), String> {
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

// ═══════════════════════════════════════════════════════════════════════════════
// Incremental change export
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_inc(opts: &EditIncCommand, snapshot_file: &str) -> Result<(), String> {
  let inc_file = ".compact-inc.cirru";
  let error_file = ".calcit-error.cirru";

  // Clear error file at the beginning
  if let Err(e) = fs::write(error_file, "") {
    eprintln!("{} Failed to clear {}: {}", "⚠".yellow(), error_file, e);
  } else {
    println!("{} Cleared {}", "→".cyan(), error_file);
  }

  if opts.added.is_empty()
    && opts.removed.is_empty()
    && opts.changed.is_empty()
    && opts.added_ns.is_empty()
    && opts.removed_ns.is_empty()
    && opts.ns_updated.is_empty()
  {
    return Err("No change hints provided. Use --added/--removed/--changed or namespace flags.".to_string());
  }

  let snapshot = load_snapshot(snapshot_file)?;

  let mut changes = ChangesDict::default();
  let mut changed_entries: HashMap<Arc<str>, FileChangeInfo> = HashMap::new();

  for ns in &opts.added_ns {
    check_ns_editable(&snapshot, ns)?;
    let file = snapshot
      .files
      .get(ns)
      .ok_or_else(|| format!("Namespace '{ns}' not found in snapshot. Did you save compact.cirru?"))?;
    changes.added.insert(Arc::from(ns.as_str()), file.clone());
  }

  for ns in &opts.removed_ns {
    check_ns_editable(&snapshot, ns)?;
    changes.removed.insert(Arc::from(ns.as_str()));
  }

  for ns in &opts.ns_updated {
    check_ns_editable(&snapshot, ns)?;
    let file = snapshot
      .files
      .get(ns)
      .ok_or_else(|| format!("Namespace '{ns}' not found in snapshot. Did you save compact.cirru?"))?;
    let entry = ensure_change_entry(&mut changed_entries, ns);
    entry.ns = Some(file.ns.code.clone());
  }

  for target in &opts.added {
    let (namespace, definition) = parse_target(target)?;
    check_ns_editable(&snapshot, namespace)?;
    let file = snapshot
      .files
      .get(namespace)
      .ok_or_else(|| format!("Namespace '{namespace}' not found in snapshot"))?;
    let code_entry = file
      .defs
      .get(definition)
      .ok_or_else(|| format!("Definition '{definition}' not found in namespace '{namespace}'"))?;
    let entry = ensure_change_entry(&mut changed_entries, namespace);
    entry.added_defs.insert(definition.to_string(), code_entry.code.clone());
  }

  for target in &opts.changed {
    let (namespace, definition) = parse_target(target)?;
    check_ns_editable(&snapshot, namespace)?;
    let file = snapshot
      .files
      .get(namespace)
      .ok_or_else(|| format!("Namespace '{namespace}' not found in snapshot"))?;
    let code_entry = file
      .defs
      .get(definition)
      .ok_or_else(|| format!("Definition '{definition}' not found in namespace '{namespace}'"))?;
    let entry = ensure_change_entry(&mut changed_entries, namespace);
    entry.changed_defs.insert(definition.to_string(), code_entry.code.clone());
  }

  for target in &opts.removed {
    let (namespace, definition) = parse_target(target)?;
    check_ns_editable(&snapshot, namespace)?;
    let entry = ensure_change_entry(&mut changed_entries, namespace);
    entry.removed_defs.insert(definition.to_string());
  }

  if !changed_entries.is_empty() {
    changes.changed = changed_entries;
  }

  if changes.added.is_empty() && changes.removed.is_empty() && changes.changed.is_empty() {
    return Err("No change data collected. Confirm the flags match definitions saved in compact.cirru.".to_string());
  }

  let namespace_total = changes.added.len() + changes.removed.len() + changes.changed.len();

  let edn_data: cirru_edn::Edn = changes
    .try_into()
    .map_err(|e| format!("Failed to serialize change dictionary: {e}"))?;
  let content = cirru_edn::format(&edn_data, true).map_err(|e| format!("Failed to format change dictionary: {e}"))?;

  fs::write(inc_file, &content).map_err(|e| format!("Failed to write {inc_file}: {e}"))?;

  println!(
    "{} Wrote incremental changes (namespaces: {}) to {}",
    "✓".green(),
    namespace_total,
    inc_file.cyan()
  );
  println!(
    "{}",
    "Watcher will process changes. Wait ~300ms then run 'cr query error' to check result."
      .to_string()
      .dimmed()
  );

  Ok(())
}

fn ensure_change_entry<'a>(changed_entries: &'a mut HashMap<Arc<str>, FileChangeInfo>, namespace: &str) -> &'a mut FileChangeInfo {
  let key: Arc<str> = Arc::from(namespace.to_string());
  changed_entries.entry(key).or_insert_with(|| FileChangeInfo {
    ns: None,
    added_defs: HashMap::new(),
    removed_defs: HashSet::new(),
    changed_defs: HashMap::new(),
  })
}

/// Print usage tips based on the import rule type
fn print_import_usage_tips(rule: &Cirru, source_ns: &str) {
  // Analyze the import rule to determine its type
  if let Cirru::List(items) = rule {
    let mut import_type = None;
    let mut symbols = Vec::new();
    let mut alias = None;

    // Parse the import rule: (namespace :refer [symbols...]) or (namespace :as alias) or (namespace :default symbol)
    let mut i = 1; // Skip the namespace (first element)
    while i < items.len() {
      if let Cirru::Leaf(tag) = &items[i] {
        match tag.as_ref() {
          ":refer" => {
            import_type = Some("refer");
            // Next item should be a list of symbols or a single symbol
            if i + 1 < items.len() {
              match &items[i + 1] {
                Cirru::List(syms) => {
                  for sym in syms {
                    if let Cirru::Leaf(s) = sym {
                      symbols.push(s.to_string());
                    }
                  }
                }
                Cirru::Leaf(s) => symbols.push(s.to_string()),
              }
            }
            break;
          }
          ":as" => {
            import_type = Some("as");
            if i + 1 < items.len() {
              if let Cirru::Leaf(a) = &items[i + 1] {
                alias = Some(a.to_string());
              }
            }
            break;
          }
          ":default" => {
            import_type = Some("default");
            if i + 1 < items.len() {
              if let Cirru::Leaf(s) = &items[i + 1] {
                symbols.push(s.to_string());
              }
            }
            break;
          }
          _ => {}
        }
      }
      i += 1;
    }

    // Print tips based on import type
    println!();
    println!("{}", "Usage tips:".dimmed());

    match import_type {
      Some("refer") => {
        if symbols.is_empty() {
          println!(
            "  {} Use imported symbols directly: {}",
            "·".dimmed(),
            "(symbol-name ...)".to_string().cyan()
          );
        } else {
          println!("  {} Use imported symbols directly:", "·".dimmed());
          for symbol in symbols.iter().take(3) {
            println!("    {}", format!("({symbol} ...)").cyan());
          }
          if symbols.len() > 3 {
            println!("    {}", format!("... and {} more", symbols.len() - 3).dimmed());
          }
        }
      }
      Some("as") => {
        if let Some(a) = alias {
          println!("  {} Use with alias: {}", "·".dimmed(), format!("({a}/symbol-name ...)").cyan());
          println!(
            "  {} List definitions: {}",
            "·".dimmed(),
            format!("cr query defs {source_ns}").cyan()
          );
        }
      }
      Some("default") => {
        if !symbols.is_empty() {
          println!(
            "  {} Default import available as: {}",
            "·".dimmed(),
            format!("({} ...)", symbols[0]).cyan()
          );
        }
      }
      None => {
        // Plain import without :refer/:as/:default
        println!(
          "  {} Use with full namespace: {}",
          "·".dimmed(),
          format!("({source_ns}/symbol-name ...)").cyan()
        );
        println!(
          "  {} List definitions: {}",
          "·".dimmed(),
          format!("cr query defs {source_ns}").cyan()
        );
      }
      _ => {
        // Unknown import type
        println!(
          "  {} Use with full namespace: {}",
          "·".dimmed(),
          format!("({source_ns}/symbol-name ...)").cyan()
        );
      }
    }
  }
}
