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
  EditAddModuleCommand, EditAddNsCommand, EditAtCommand, EditCommand, EditConfigCommand, EditDefCommand, EditDocCommand,
  EditExamplesCommand, EditImportsCommand, EditNsDocCommand, EditRequireCommand, EditRmDefCommand, EditRmModuleCommand,
  EditRmNsCommand, EditRmRequireCommand, EditSubcommand,
};
use calcit::snapshot::{self, CodeEntry, FileInSnapShot, Snapshot, save_snapshot_to_file};
use cirru_parser::Cirru;
use colored::Colorize;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::sync::Arc;

/// Parse "namespace/definition" format into (namespace, definition)
fn parse_target(target: &str) -> Result<(&str, &str), String> {
  target
    .rsplit_once('/')
    .ok_or_else(|| format!("Invalid target format: '{target}'. Expected 'namespace/definition' (e.g. 'app.core/main')"))
}

pub fn handle_edit_command(cmd: &EditCommand, snapshot_file: &str) -> Result<(), String> {
  match &cmd.subcommand {
    EditSubcommand::Def(opts) => handle_def(opts, snapshot_file),
    EditSubcommand::RmDef(opts) => handle_rm_def(opts, snapshot_file),
    EditSubcommand::Doc(opts) => handle_doc(opts, snapshot_file),
    EditSubcommand::Examples(opts) => handle_examples(opts, snapshot_file),
    EditSubcommand::At(opts) => handle_at(opts, snapshot_file),
    EditSubcommand::AddNs(opts) => handle_add_ns(opts, snapshot_file),
    EditSubcommand::RmNs(opts) => handle_rm_ns(opts, snapshot_file),
    EditSubcommand::Imports(opts) => handle_imports(opts, snapshot_file),
    EditSubcommand::Require(opts) => handle_require(opts, snapshot_file),
    EditSubcommand::RmRequire(opts) => handle_rm_require(opts, snapshot_file),
    EditSubcommand::NsDoc(opts) => handle_ns_doc(opts, snapshot_file),
    EditSubcommand::AddModule(opts) => handle_add_module(opts, snapshot_file),
    EditSubcommand::RmModule(opts) => handle_rm_module(opts, snapshot_file),
    EditSubcommand::Config(opts) => handle_config(opts, snapshot_file),
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

/// Check if namespace belongs to the current package (can be edited)
fn check_ns_editable(snapshot: &Snapshot, namespace: &str) -> Result<(), String> {
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

/// Determine input mode and parse raw input string into a `Cirru` node.
/// - If `inline_json` is Some -> parse that JSON string.
/// - Else if `json_input` is true -> parse `raw` as JSON.
/// - Else -> parse `raw` as Cirru text (default).
fn parse_input_to_cirru(raw: &str, inline_json: &Option<String>, json_input: bool) -> Result<Cirru, String> {
  // If inline JSON provided, use it (takes precedence)
  if let Some(j) = inline_json {
    return json_to_cirru(j);
  }

  if json_input {
    return json_to_cirru(raw);
  }

  // Default: parse as cirru text
  let parsed = cirru_parser::parse(raw).map_err(|e| {
    let mut msg = format!("Failed to parse Cirru text: {e}");
    msg.push_str("\nTips: If your input contains special characters like '|' or '$', ensure the shell does not strip them — wrap input in single quotes or use --file/--stdin.\n");
    msg.push_str("If you intended to provide JSON, pass --json-input or use -j for inline JSON.");
    msg
  })?;

  if parsed.len() == 1 {
    Ok(parsed.into_iter().next().unwrap())
  } else {
    Ok(Cirru::List(parsed))
  }
}

/// Parse JSON string to Cirru syntax tree
fn json_to_cirru(json_str: &str) -> Result<Cirru, String> {
  let json: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
    format!("Failed to parse JSON: {e}. If this is Cirru text, omit --json-input or use --cirru; for inline Cirru prefer --file or --stdin to avoid shell escaping.")
  })?;

  match json_value_to_cirru(&json) {
    Ok(c) => Ok(c),
    Err(e) => Err(format!(
      "{e} If your input is Cirru source, try passing it as Cirru (omit --json-input or use --cirru)."
    )),
  }
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
    serde_json::Value::Object(_) => Err(
      "JSON objects cannot be converted to Cirru syntax tree. Consider providing an array or string, or use Cirru source format."
        .to_string(),
    ),
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

fn handle_def(opts: &EditDefCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let raw = read_code_input(&opts.file, &opts.json, opts.stdin)?.ok_or("Code input required: use --file, --json, or --stdin")?;

  let syntax_tree = parse_input_to_cirru(&raw, &opts.json, opts.json_input)?;

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

  if exists && !opts.replace {
    return Err(format!(
      "Definition '{definition}' already exists in namespace '{namespace}'. Use --replace to overwrite."
    ));
  }

  // Create or update definition
  let code_entry = CodeEntry::from_code(syntax_tree);
  file_data.defs.insert(definition.to_string(), code_entry);

  save_snapshot(&snapshot, snapshot_file)?;

  if exists {
    println!(
      "{} Updated definition '{}' in namespace '{}'",
      "✓".green(),
      definition.cyan(),
      namespace
    );
  } else {
    println!(
      "{} Created definition '{}' in namespace '{}'",
      "✓".green(),
      definition.cyan(),
      namespace
    );
  }

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
  let code_input = read_code_input(&opts.file, &opts.json, opts.stdin)?;
  let raw = code_input
    .as_deref()
    .ok_or("Examples input required: use --file, --json, --stdin, or --clear")?;

  // Parse examples - expect an array of Cirru expressions
  let examples: Vec<Cirru> = if opts.json.is_some() || opts.json_input {
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

fn handle_at(opts: &EditAtCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let path = parse_path(&opts.path)?;

  // For delete operation, code input is not required
  let code_input = read_code_input(&opts.file, &opts.json, opts.stdin)?;

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
    .ok_or_else(|| format!("Definition '{definition}' not found"))?;

  // Prepare parsed new node (if applicable)
  let new_node_opt: Option<Cirru> = match opts.operation.as_str() {
    "delete" => None,
    _ => {
      let raw = code_input
        .as_deref()
        .ok_or("Code input required for this operation: use --file, --json, or --stdin")?;
      Some(parse_input_to_cirru(raw, &opts.json, opts.json_input)?)
    }
  };

  // Apply operation at path
  let new_code = apply_operation_at_path(&code_entry.code, &path, &opts.operation, new_node_opt.as_ref())?;

  code_entry.code = new_code.clone();

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Applied '{}' at path [{}] in '{}/{}'",
    "✓".green(),
    opts.operation.yellow(),
    opts.path,
    namespace,
    definition.cyan()
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

fn apply_operation_at_path(code: &Cirru, path: &[usize], operation: &str, new_node: Option<&Cirru>) -> Result<Cirru, String> {
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

  // Check if namespace can be edited (must be under current package)
  check_ns_editable(&snapshot, &opts.namespace)?;

  if snapshot.files.contains_key(&opts.namespace) {
    return Err(format!("Namespace '{}' already exists", opts.namespace));
  }

  // Create ns code
  let ns_code = if let Some(raw) = read_code_input(&opts.file, &opts.json, opts.stdin)? {
    parse_input_to_cirru(&raw, &opts.json, opts.json_input)?
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
  let raw = read_code_input(&opts.file, &opts.json, opts.stdin)?.ok_or("Imports input required: use --file, --json, or --stdin")?;

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
    let cirru_node = parse_input_to_cirru(&raw, &opts.json, opts.json_input)?;
    fn cirru_to_json_value(c: &Cirru) -> serde_json::Value {
      match c {
        Cirru::Leaf(s) => serde_json::Value::String(s.to_string()),
        Cirru::List(items) => serde_json::Value::Array(items.iter().map(cirru_to_json_value).collect()),
      }
    }

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
        let import_cirru = json_value_to_cirru(&import)?;
        ns_code_items.push(import_cirru);
      }
    }
  } else {
    return Err("Imports must be a JSON/Cirru array (e.g. [(require ...)]).".to_string());
  }

  file_data.ns.code = Cirru::List(ns_code_items);

  save_snapshot(&snapshot, snapshot_file)?;

  println!("{} Updated imports for namespace '{}'", "✓".green(), opts.namespace.cyan());

  Ok(())
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

fn handle_require(opts: &EditRequireCommand, snapshot_file: &str) -> Result<(), String> {
  let raw =
    read_code_input(&opts.file, &opts.json, opts.stdin)?.ok_or("Require rule input required: use --file, --json, or --stdin")?;

  let new_rule = parse_input_to_cirru(&raw, &opts.json, opts.json_input)?;

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
      rules[idx] = new_rule;
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
    rules.push(new_rule);
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

  Ok(())
}

fn handle_rm_require(opts: &EditRmRequireCommand, snapshot_file: &str) -> Result<(), String> {
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
