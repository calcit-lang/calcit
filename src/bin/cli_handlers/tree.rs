use cirru_parser::Cirru;
use colored::Colorize;
use std::fs;
use std::io::{self, Read};
use std::sync::Arc;

use super::common::{ERR_CODE_INPUT_REQUIRED, json_value_to_cirru, parse_path, validate_input_flags, validate_input_sources};
use crate::cli_args::{
  TreeAppendChildCommand, TreeCommand, TreeDeleteCommand, TreeInsertAfterCommand, TreeInsertBeforeCommand, TreeInsertChildCommand,
  TreeReplaceCommand, TreeShowCommand, TreeSubcommand, TreeSwapNextCommand, TreeSwapPrevCommand, TreeWrapCommand,
};

// Import shared functions from edit module
use super::edit::{
  apply_operation_at_path, check_ns_editable, load_snapshot, navigate_to_path, parse_target, process_node_with_references,
  save_snapshot,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Helper functions
// ═══════════════════════════════════════════════════════════════════════════════

/// Check if a Cirru node is a single-element list containing only a string leaf,
/// which might confuse LLM thinking it's a leaf node when it's actually an expression.
fn warn_if_single_string_expression(node: &Cirru, input_source: &str) {
  if let Cirru::List(items) = node {
    if items.len() == 1 {
      if let Some(Cirru::Leaf(_)) = items.first() {
        eprintln!("\n⚠️  Note: Cirru one-liner input '{input_source}' was parsed as an expression (list with one element).");
        eprintln!("   In Cirru syntax, this creates a list containing one element.");
        eprintln!("   If you want a leaf node (plain string), use --json-leaf parameter.");
        eprintln!("   Example: --json-leaf -e '{input_source}' creates a leaf, not an expression.\n");
      }
    }
  }
}

/// Read code input from file, inline code, or stdin.
fn read_code_input(file: &Option<String>, code: &Option<String>, stdin: bool) -> Result<Option<String>, String> {
  let sources = [stdin, file.is_some(), code.is_some()];
  validate_input_sources(&sources)?;

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
fn parse_input_to_cirru(input: &str, json_opt: &Option<String>, json_input: bool, json_leaf: bool) -> Result<Cirru, String> {
  // Check for conflicting flags (tree commands only support one-liner Cirru now)
  validate_input_flags(json_leaf, json_input, false)?;

  // If json_leaf is set, wrap input directly as a leaf node
  if json_leaf {
    return Ok(Cirru::Leaf(Arc::from(input)));
  }

  // Determine if we should parse as JSON (must be explicitly specified)
  let use_json = json_opt.is_some() || json_input;

  if use_json {
    // Parse as JSON array
    let json_value: serde_json::Value = serde_json::from_str(input).map_err(|e| format!("Failed to parse JSON: {e}"))?;
    json_value_to_cirru(&json_value)
  } else {
    // Parse as Cirru one-liner only
    let result = cirru_parser::parse_expr_one_liner(input).map_err(|e| format!("Failed to parse Cirru one-liner: {e}"))?;
    warn_if_single_string_expression(&result, input);
    Ok(result)
  }
}

/// Format a Cirru node for preview display
fn format_preview(node: &Cirru, max_lines: usize) -> String {
  let formatted = match node {
    Cirru::Leaf(s) => {
      // For leaf nodes, just show the value
      format!("  {:?}", s.as_ref())
    }
    Cirru::List(_) => {
      // For list nodes, use the Cirru formatter
      match cirru_parser::format(std::slice::from_ref(node), cirru_parser::CirruWriterOptions { use_inline: false }) {
        Ok(cirru_str) => {
          let lines: Vec<&str> = cirru_str.lines().collect();
          if lines.len() > max_lines {
            let mut result = String::new();
            for line in lines.iter().take(max_lines) {
              result.push_str("  ");
              result.push_str(line);
              result.push('\n');
            }
            result.push_str(&format!("  {}\n", format!("... ({} more lines)", lines.len() - max_lines).dimmed()));
            result
          } else {
            lines.iter().map(|line| format!("  {line}\n")).collect()
          }
        }
        Err(e) => format!("  {}\n", format!("(failed to format: {e})").red()),
      }
    }
  };
  formatted.trim_end().to_string()
}

/// Format a Cirru node for preview display with type annotation for short content
fn format_preview_with_type(node: &Cirru, max_lines: usize) -> String {
  let base_preview = format_preview(node, max_lines);

  // Add type annotation for short content (especially single-element expressions)
  let type_label = match node {
    Cirru::Leaf(_) => " (leaf)".dimmed().to_string(),
    Cirru::List(items) => {
      if items.len() == 1 {
        " (expr)".dimmed().to_string()
      } else {
        String::new()
      }
    }
  };

  if type_label.is_empty() {
    base_preview
  } else {
    format!("{base_preview}{type_label}")
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
        "tree replace".cyan(),
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

  let raw = code_input.as_deref().ok_or(ERR_CODE_INPUT_REQUIRED)?;

  let new_node = parse_input_to_cirru(raw, &opts.json, opts.json_input, opts.json_leaf)?;

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

  // Save original for comparison
  let old_node = navigate_to_path(&code_entry.code, &path)?;

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
  println!("{}:", "From".yellow().bold());
  println!("{}", format_preview_with_type(&old_node, 20));
  println!();
  println!("{}:", "To".green().bold());
  let new_node = navigate_to_path(&new_code, &path)?;
  println!("{}", format_preview_with_type(&new_node, 20));

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

  // Save original node and parent for comparison
  let old_node = navigate_to_path(&code_entry.code, &path)?;
  let parent_path: Vec<usize> = if path.is_empty() { vec![] } else { path[..path.len() - 1].to_vec() };

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
  println!();
  println!("{}:", "Deleted node".yellow().bold());
  println!("{}", format_preview_with_type(&old_node, 20));
  println!();
  println!("{}:", "Parent after deletion".green().bold());
  let new_parent = if parent_path.is_empty() {
    new_code.clone()
  } else {
    navigate_to_path(&new_code, &parent_path)?
  };
  println!("{}", format_preview_with_type(&new_parent, 20));

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
  _depth: usize,
) -> Result<(), String> {
  let (namespace, definition) = parse_target(target)?;
  let path = parse_path(path_str)?;

  let code_input = read_code_input(opts.file(), opts.code(), opts.stdin())?;

  let raw = code_input.as_deref().ok_or(ERR_CODE_INPUT_REQUIRED)?;

  let new_node = parse_input_to_cirru(raw, opts.json(), opts.json_input(), opts.json_leaf())?;

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

  // Save parent before insertion for comparison
  let parent_path: Vec<usize> = if path.is_empty() { vec![] } else { path[..path.len() - 1].to_vec() };
  let old_parent = if parent_path.is_empty() {
    code_entry.code.clone()
  } else {
    navigate_to_path(&code_entry.code, &parent_path)?
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
  println!("{}:", "Inserted node".cyan().bold());
  println!("{}", format_preview_with_type(&processed_node, 10));
  println!();
  println!("{}:", "Parent before".yellow().bold());
  println!("{}", format_preview_with_type(&old_parent, 15));
  println!();
  println!("{}:", "Parent after".green().bold());
  let new_parent = if parent_path.is_empty() {
    new_code.clone()
  } else {
    navigate_to_path(&new_code, &parent_path)?
  };
  println!("{}", format_preview_with_type(&new_parent, 15));

  Ok(())
}

fn handle_swap_next(opts: &TreeSwapNextCommand, snapshot_file: &str) -> Result<(), String> {
  generic_swap_handler(&opts.target, &opts.path, "swap-next-sibling", snapshot_file, opts.depth)
}

fn handle_swap_prev(opts: &TreeSwapPrevCommand, snapshot_file: &str) -> Result<(), String> {
  generic_swap_handler(&opts.target, &opts.path, "swap-prev-sibling", snapshot_file, opts.depth)
}

fn generic_swap_handler(target: &str, path_str: &str, operation: &str, snapshot_file: &str, _depth: usize) -> Result<(), String> {
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

  // Save parent before swap for comparison
  let parent_path: Vec<usize> = if path.is_empty() { vec![] } else { path[..path.len() - 1].to_vec() };
  let old_parent = if parent_path.is_empty() {
    code_entry.code.clone()
  } else {
    navigate_to_path(&code_entry.code, &parent_path)?
  };

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
  println!("{}:", "Parent before swap".yellow().bold());
  println!("{}", format_preview_with_type(&old_parent, 15));
  println!();
  println!("{}:", "Parent after swap".green().bold());
  let new_parent = if parent_path.is_empty() {
    new_code.clone()
  } else {
    navigate_to_path(&new_code, &parent_path)?
  };
  println!("{}", format_preview_with_type(&new_parent, 15));

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
