use cirru_parser::Cirru;
use colored::Colorize;
use std::fs;
use std::io::{self, Read};
use std::sync::Arc;

use crate::cli_args::{
  TreeAppendChildCommand, TreeCommand, TreeDeleteCommand, TreeInsertAfterCommand, TreeInsertBeforeCommand, TreeInsertChildCommand,
  TreeReplaceCommand, TreeShowCommand, TreeSubcommand, TreeSwapNextCommand, TreeSwapPrevCommand, TreeWrapCommand,
};

// Import shared functions from edit module
use super::edit::{apply_operation_at_path, check_ns_editable, load_snapshot, navigate_to_path, parse_path, parse_target, process_node_with_references, save_snapshot};

// ═══════════════════════════════════════════════════════════════════════════════
// Helper functions
// ═══════════════════════════════════════════════════════════════════════════════

/// Read code input from file, inline code, or stdin.
fn read_code_input(file: &Option<String>, code: &Option<String>, stdin: bool) -> Result<Option<String>, String> {
  let sources = stdin as usize + file.is_some() as usize + code.is_some() as usize;
  if sources > 1 {
    return Err("Multiple input sources provided. Use only one of: --stdin/-s, --file/-f, --code/-e.".to_string());
  }

  if stdin {
    let mut buffer = String::new();
    io::stdin()
      .read_to_string(&mut buffer)
      .map_err(|e| format!("Failed to read from stdin: {e}"))?;
    Ok(Some(buffer.trim().to_string()))
  } else if let Some(path) = file {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file '{path}': {e}"))?;
    Ok(Some(content.trim().to_string()))
  } else if let Some(s) = code {
    Ok(Some(s.trim().to_string()))
  } else {
    Ok(None)
  }
}


/// Main handler for code command
pub fn handle_tree_command(cmd: &TreeCommand, snapshot_file: &str) -> Result<(), String> {
  match &cmd.subcommand {
    TreeSubcommand::Show(opts) => handle_show(opts, snapshot_file),
    TreeSubcommand::Replace(opts) => handle_replace(opts, snapshot_file),
    TreeSubcommand::Delete(opts) => handle_delete(opts, snapshot_file),
    TreeSubcommand::InsertBefore(opts) => handle_insert_before(opts, snapshot_file),
    TreeSubcommand::InsertAfter(opts) => handle_insert_after(opts, snapshot_file),
    TreeSubcommand::InsertChild(opts) => handle_insert_child(opts, snapshot_file),
    TreeSubcommand::AppendChild(opts) => handle_append_child(opts, snapshot_file),
    TreeSubcommand::SwapNext(opts) => handle_swap_next(opts, snapshot_file),
    TreeSubcommand::SwapPrev(opts) => handle_swap_prev(opts, snapshot_file),
    TreeSubcommand::Wrap(opts) => handle_wrap(opts, snapshot_file),
  }
}

/// Parse input to Cirru node
fn parse_input_to_cirru(
  input: &str,
  json_opt: &Option<String>,
  json_input: bool,
  cirru_multiline: bool,
  json_leaf: bool,
) -> Result<Cirru, String> {
  // Check for conflicting flags
  if json_leaf && (json_input || cirru_multiline) {
    return Err("Conflicting input flags: use only one of --json-leaf, --json-input, or --cirru.".to_string());
  }

  // If json_leaf is set, wrap input directly as a leaf node
  if json_leaf {
    return Ok(Cirru::Leaf(Arc::from(input)));
  }

  // Determine if we should parse as JSON (must be explicitly specified)
  let use_json = json_opt.is_some() || json_input;

  if use_json {
    // Parse as JSON array
    let json_value: serde_json::Value = serde_json::from_str(input).map_err(|e| format!("Failed to parse JSON: {e}"))?;
    json_to_cirru(&json_value)
  } else {
    // Parse as Cirru (default: one-liner format, unless --cirru is set)
    // cirru_parser::parse() returns Vec<Cirru>
    // cirru_parser::parse_expr_one_liner() returns Cirru (single expression without wrapper)
    if cirru_multiline {
      // Full parse: expect exactly one top-level expression
      let nodes = cirru_parser::parse(input)
        .map_err(|e| format!("Failed to parse Cirru: {}", e.format_detailed(Some(input))))?;
      if nodes.len() != 1 {
        return Err(format!("Expected single Cirru expression, got {}", nodes.len()));
      }
      Ok(nodes[0].clone())
    } else {
      // One-liner: parse single expression directly (default for code input)
      cirru_parser::parse_expr_one_liner(input)
        .map_err(|e| format!("Failed to parse Cirru one-liner: {e}"))
    }
  }
}

/// Convert JSON to Cirru
fn json_to_cirru(json: &serde_json::Value) -> Result<Cirru, String> {
  match json {
    serde_json::Value::String(s) => Ok(Cirru::Leaf(Arc::from(s.as_str()))),
    serde_json::Value::Number(n) => Ok(Cirru::Leaf(Arc::from(n.to_string()))),
    serde_json::Value::Bool(b) => Ok(Cirru::Leaf(Arc::from(b.to_string()))),
    serde_json::Value::Null => Ok(Cirru::Leaf(Arc::from("null"))),
    serde_json::Value::Array(arr) => {
      let items: Result<Vec<Cirru>, String> = arr.iter().map(json_to_cirru).collect();
      Ok(Cirru::List(items?))
    }
    serde_json::Value::Object(_) => Err("JSON objects not supported, use arrays".to_string()),
  }
}

/// Pretty print Cirru node
fn format_cirru_preview(node: &Cirru, depth: usize, max_depth: usize, indent: usize) -> String {
  if max_depth > 0 && depth >= max_depth {
    return "...".to_string();
  }

  let indent_str = "  ".repeat(indent);

  match node {
    Cirru::Leaf(s) => format!("{}{:?}", indent_str, s.as_ref()),
    Cirru::List(items) => {
      if items.is_empty() {
        return format!("{indent_str}[]");
      }

      let mut result = format!("{indent_str}[\n");
      for (i, item) in items.iter().enumerate() {
        if max_depth > 0 && depth + 1 >= max_depth && i >= 3 {
          result.push_str(&format!("{}  ...{} more\n", indent_str, items.len() - i));
          break;
        }
        result.push_str(&format_cirru_preview(item, depth + 1, max_depth, indent + 1));
        if i < items.len() - 1 {
          result.push(',');
        }
        result.push('\n');
      }
      result.push_str(&format!("{indent_str}]"));
      result
    }
  }
}

// ============================================================================
// Command handlers
// ============================================================================

fn handle_show(opts: &TreeShowCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;
  let path = parse_path(&opts.path)?;

  let snapshot = load_snapshot(snapshot_file)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found"))?;

  let node = navigate_to_path(&code_entry.code, &path)?;

  // Print info
  println!(
    "{}: {}  [{}]",
    "At".green().bold(),
    format!("{namespace}/{definition}").cyan(),
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
  );

  let node_type = match &node {
    Cirru::Leaf(_) => "leaf",
    Cirru::List(items) => {
      println!("{}: {} ({} items)", "Type".green().bold(), "list".yellow(), items.len());
      println!();
      println!("{}:", "Cirru preview".green().bold());
      println!("  ");
      let cirru_str = cirru_parser::format(std::slice::from_ref(&node), cirru_parser::CirruWriterOptions { use_inline: true })
        .map_err(|e| format!("Failed to format Cirru: {e}"))?;
      for line in cirru_str.lines() {
        println!("  {line}");
      }
      println!();

      if !items.is_empty() {
        println!("{}:", "Children".green().bold());
        for (i, item) in items.iter().enumerate() {
          let type_str = match item {
            Cirru::Leaf(s) => format!("{:?}", s.as_ref()),
            Cirru::List(children) => format!("({} items)", children.len()),
          };
          println!(
            "  [{}] {} {} -p {:?}",
            i,
            type_str.yellow(),
            "->".dimmed(),
            format!("{},{}", opts.path, i)
          );
        }
        println!();
      }

      println!("{}:", "JSON".green().bold());
      println!("{}", cirru_to_json(&node));
      if opts.depth > 0 {
        println!("{}", format!("(depth limited to {})", opts.depth).dimmed());
      }
      println!();

      println!(
        "{}: To modify, use `{} {} -p \"{}\" '<cirru>'`",
        "Tip".blue().bold(),
        "code replace".cyan(),
        opts.target,
        opts.path
      );
      println!("     Use `{}` for JSON input.", "-j '<json>'".to_string().cyan());

      return Ok(());
    }
  };

  if matches!(node_type, "leaf") {
    println!("{}: {}", "Type".green().bold(), "leaf".yellow());
    if let Cirru::Leaf(s) = &node {
      println!("{}: {:?}", "Value".green().bold(), s.as_ref());
    }
  }

  Ok(())
}

fn handle_replace(opts: &TreeReplaceCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;
  let path = parse_path(&opts.path)?;

  let code_input = read_code_input(&opts.file, &opts.code, opts.stdin)?;

  let raw = code_input
    .as_deref()
    .ok_or("Code input required: use --file, --code, --json, or --stdin")?;

  let new_node = parse_input_to_cirru(
    raw,
    &opts.json,
    opts.json_input,
    opts.cirru,
    opts.json_leaf,
  )?;

  let mut snapshot = load_snapshot(snapshot_file)?;
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get_mut(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found"))?;

  // Get original node for reference if needed
  let original_node = if opts.refer_original.is_some() || opts.refer_inner_branch.is_some() {
    Some(navigate_to_path(&code_entry.code, &path)?)
  } else {
    None
  };

  // Process node with replacements if needed
  let processed_node = if opts.refer_original.is_some() || opts.refer_inner_branch.is_some() {
    process_node_with_references(
      &new_node,
      original_node.as_ref(),
      &opts.refer_original,
      &opts.refer_inner_branch,
      &opts.refer_inner_placeholder,
    )?
  } else {
    new_node
  };

  let new_code = apply_operation_at_path(&code_entry.code, &path, "replace", Some(&processed_node))?;
  code_entry.code = new_code.clone();

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Applied 'replace' at path [{}] in '{}/{}'",
    "✓".green(),
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
    namespace,
    definition
  );
  println!();
  println!("{}:", "Preview".green().bold());
  println!("{}", format_cirru_preview(&navigate_to_path(&new_code, &path)?, 0, opts.depth, 0));
  if opts.depth > 0 {
    println!("{}", format!("(depth limited to {})", opts.depth).dimmed());
  }

  Ok(())
}

fn handle_delete(opts: &TreeDeleteCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;
  let path = parse_path(&opts.path)?;

  let mut snapshot = load_snapshot(snapshot_file)?;
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get_mut(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found"))?;

  let new_code = apply_operation_at_path(&code_entry.code, &path, "delete", None)?;
  code_entry.code = new_code.clone();

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Deleted node at path [{}] in '{}/{}'",
    "✓".green(),
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
    namespace,
    definition
  );

  Ok(())
}

fn handle_insert_before(opts: &TreeInsertBeforeCommand, snapshot_file: &str) -> Result<(), String> {
  generic_insert_handler(&opts.target, &opts.path, "insert-before", opts, snapshot_file, opts.depth)
}

fn handle_insert_after(opts: &TreeInsertAfterCommand, snapshot_file: &str) -> Result<(), String> {
  generic_insert_handler(&opts.target, &opts.path, "insert-after", opts, snapshot_file, opts.depth)
}

fn handle_insert_child(opts: &TreeInsertChildCommand, snapshot_file: &str) -> Result<(), String> {
  generic_insert_handler(&opts.target, &opts.path, "insert-child", opts, snapshot_file, opts.depth)
}

fn handle_append_child(opts: &TreeAppendChildCommand, snapshot_file: &str) -> Result<(), String> {
  generic_insert_handler(&opts.target, &opts.path, "append-child", opts, snapshot_file, opts.depth)
}

// Generic trait for insert-like operations
trait InsertOperation {
  fn file(&self) -> &Option<String>;
  fn code(&self) -> &Option<String>;
  fn json(&self) -> &Option<String>;
  fn stdin(&self) -> bool;
  fn json_input(&self) -> bool;
  fn cirru(&self) -> bool;
  fn json_leaf(&self) -> bool;
  fn refer_original(&self) -> &Option<String>;
  fn refer_inner_branch(&self) -> &Option<String>;
  fn refer_inner_placeholder(&self) -> &Option<String>;
}

impl InsertOperation for TreeInsertBeforeCommand {
  fn file(&self) -> &Option<String> {
    &self.file
  }
  fn code(&self) -> &Option<String> {
    &self.code
  }
  fn json(&self) -> &Option<String> {
    &self.json
  }
  fn stdin(&self) -> bool {
    self.stdin
  }
  fn json_input(&self) -> bool {
    self.json_input
  }
  fn cirru(&self) -> bool {
    self.cirru
  }
  fn json_leaf(&self) -> bool {
    self.json_leaf
  }
  fn refer_original(&self) -> &Option<String> {
    &self.refer_original
  }
  fn refer_inner_branch(&self) -> &Option<String> {
    &self.refer_inner_branch
  }
  fn refer_inner_placeholder(&self) -> &Option<String> {
    &self.refer_inner_placeholder
  }
}

impl InsertOperation for TreeInsertAfterCommand {
  fn file(&self) -> &Option<String> {
    &self.file
  }
  fn code(&self) -> &Option<String> {
    &self.code
  }
  fn json(&self) -> &Option<String> {
    &self.json
  }
  fn stdin(&self) -> bool {
    self.stdin
  }
  fn json_input(&self) -> bool {
    self.json_input
  }
  fn cirru(&self) -> bool {
    self.cirru
  }
  fn json_leaf(&self) -> bool {
    self.json_leaf
  }
  fn refer_original(&self) -> &Option<String> {
    &self.refer_original
  }
  fn refer_inner_branch(&self) -> &Option<String> {
    &self.refer_inner_branch
  }
  fn refer_inner_placeholder(&self) -> &Option<String> {
    &self.refer_inner_placeholder
  }
}

impl InsertOperation for TreeInsertChildCommand {
  fn file(&self) -> &Option<String> {
    &self.file
  }
  fn code(&self) -> &Option<String> {
    &self.code
  }
  fn json(&self) -> &Option<String> {
    &self.json
  }
  fn stdin(&self) -> bool {
    self.stdin
  }
  fn json_input(&self) -> bool {
    self.json_input
  }
  fn cirru(&self) -> bool {
    self.cirru
  }
  fn json_leaf(&self) -> bool {
    self.json_leaf
  }
  fn refer_original(&self) -> &Option<String> {
    &self.refer_original
  }
  fn refer_inner_branch(&self) -> &Option<String> {
    &self.refer_inner_branch
  }
  fn refer_inner_placeholder(&self) -> &Option<String> {
    &self.refer_inner_placeholder
  }
}

impl InsertOperation for TreeAppendChildCommand {
  fn file(&self) -> &Option<String> {
    &self.file
  }
  fn code(&self) -> &Option<String> {
    &self.code
  }
  fn json(&self) -> &Option<String> {
    &self.json
  }
  fn stdin(&self) -> bool {
    self.stdin
  }
  fn json_input(&self) -> bool {
    self.json_input
  }
  fn cirru(&self) -> bool {
    self.cirru
  }
  fn json_leaf(&self) -> bool {
    self.json_leaf
  }
  fn refer_original(&self) -> &Option<String> {
    &self.refer_original
  }
  fn refer_inner_branch(&self) -> &Option<String> {
    &self.refer_inner_branch
  }
  fn refer_inner_placeholder(&self) -> &Option<String> {
    &self.refer_inner_placeholder
  }
}

impl InsertOperation for TreeWrapCommand {
  fn file(&self) -> &Option<String> {
    &self.file
  }
  fn code(&self) -> &Option<String> {
    &self.code
  }
  fn json(&self) -> &Option<String> {
    &self.json
  }
  fn stdin(&self) -> bool {
    self.stdin
  }
  fn json_input(&self) -> bool {
    self.json_input
  }
  fn cirru(&self) -> bool {
    self.cirru
  }
  fn json_leaf(&self) -> bool {
    self.json_leaf
  }
  fn refer_original(&self) -> &Option<String> {
    &self.refer_original
  }
  fn refer_inner_branch(&self) -> &Option<String> {
    &self.refer_inner_branch
  }
  fn refer_inner_placeholder(&self) -> &Option<String> {
    &self.refer_inner_placeholder
  }
}

fn generic_insert_handler<T: InsertOperation>(
  target: &str,
  path_str: &str,
  operation: &str,
  opts: &T,
  snapshot_file: &str,
  depth: usize,
) -> Result<(), String> {
  let (namespace, definition) = parse_target(target)?;
  let path = parse_path(path_str)?;

  let code_input = read_code_input(opts.file(), opts.code(), opts.stdin())?;

  let raw = code_input
    .as_deref()
    .ok_or("Code input required: use --file, --code, --json, or --stdin")?;

  let new_node = parse_input_to_cirru(
    raw,
    opts.json(),
    opts.json_input(),
    opts.cirru(),
    opts.json_leaf(),
  )?;

  let mut snapshot = load_snapshot(snapshot_file)?;
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get_mut(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found"))?;

  // Get original node for reference if needed
  let original_node = if opts.refer_original().is_some() || opts.refer_inner_branch().is_some() {
    Some(navigate_to_path(&code_entry.code, &path)?)
  } else {
    None
  };

  // Process node with replacements if needed
  let processed_node = if opts.refer_original().is_some() || opts.refer_inner_branch().is_some() {
    process_node_with_references(
      &new_node,
      original_node.as_ref(),
      opts.refer_original(),
      opts.refer_inner_branch(),
      opts.refer_inner_placeholder(),
    )?
  } else {
    new_node
  };

  let new_code = apply_operation_at_path(&code_entry.code, &path, operation, Some(&processed_node))?;
  code_entry.code = new_code.clone();

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Applied '{}' at path [{}] in '{}/{}'",
    "✓".green(),
    operation,
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
    namespace,
    definition
  );
  println!();
  println!("{}:", "Preview".green().bold());
  println!("{}", format_cirru_preview(&navigate_to_path(&new_code, &path)?, 0, depth, 0));
  if depth > 0 {
    println!("{}", format!("(depth limited to {depth})").dimmed());
  }

  Ok(())
}

fn handle_swap_next(opts: &TreeSwapNextCommand, snapshot_file: &str) -> Result<(), String> {
  generic_swap_handler(&opts.target, &opts.path, "swap-next-sibling", snapshot_file, opts.depth)
}

fn handle_swap_prev(opts: &TreeSwapPrevCommand, snapshot_file: &str) -> Result<(), String> {
  generic_swap_handler(&opts.target, &opts.path, "swap-prev-sibling", snapshot_file, opts.depth)
}

fn generic_swap_handler(target: &str, path_str: &str, operation: &str, snapshot_file: &str, depth: usize) -> Result<(), String> {
  let (namespace, definition) = parse_target(target)?;
  let path = parse_path(path_str)?;

  let mut snapshot = load_snapshot(snapshot_file)?;
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get_mut(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found"))?;

  let new_code = apply_operation_at_path(&code_entry.code, &path, operation, None)?;
  code_entry.code = new_code.clone();

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Applied '{}' at path [{}] in '{}/{}'",
    "✓".green(),
    operation,
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
    namespace,
    definition
  );
  println!();
  println!("{}:", "Preview".green().bold());
  println!("{}", format_cirru_preview(&navigate_to_path(&new_code, &path)?, 0, depth, 0));
  if depth > 0 {
    println!("{}", format!("(depth limited to {depth})").dimmed());
  }

  Ok(())
}

fn handle_wrap(opts: &TreeWrapCommand, snapshot_file: &str) -> Result<(), String> {
  generic_insert_handler(&opts.target, &opts.path, "replace", opts, snapshot_file, opts.depth)
}

// Helper to convert Cirru to JSON string
fn cirru_to_json(node: &Cirru) -> String {
  match node {
    Cirru::Leaf(s) => serde_json::to_string(s.as_ref()).unwrap_or_else(|_| format!("\"{s}\"")),
    Cirru::List(items) => {
      let json_items: Vec<String> = items.iter().map(cirru_to_json).collect();
      format!("[{}]", json_items.join(","))
    }
  }
}
