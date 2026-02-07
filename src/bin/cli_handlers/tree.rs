use cirru_parser::Cirru;
use colored::Colorize;

use super::common::{ERR_CODE_INPUT_REQUIRED, cirru_to_json, parse_input_to_cirru, parse_path, read_code_input};
use super::tips::{Tips, tip_prefer_oneliner_json, tip_root_edit};
use crate::cli_args::{
  TreeAppendChildCommand, TreeCommand, TreeCpCommand, TreeDeleteCommand, TreeInsertAfterCommand, TreeInsertBeforeCommand,
  TreeInsertChildCommand, TreeReplaceCommand, TreeReplaceLeafCommand, TreeShowCommand, TreeSubcommand, TreeSwapNextCommand,
  TreeSwapPrevCommand, TreeTargetReplaceCommand, TreeWrapCommand,
};

// Import shared functions from edit module
use super::edit::{
  apply_operation_at_path, check_ns_editable, load_snapshot, navigate_to_path, parse_target, process_node_with_references,
  save_snapshot,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Helper functions
// ═══════════════════════════════════════════════════════════════════════════════

/// Main handler for code command
pub fn handle_tree_command(cmd: &TreeCommand, snapshot_file: &str) -> Result<(), String> {
  match &cmd.subcommand {
    TreeSubcommand::Show(opts) => handle_show(opts, snapshot_file, opts.json),
    TreeSubcommand::Replace(opts) => handle_replace(opts, snapshot_file),
    TreeSubcommand::ReplaceLeaf(opts) => handle_replace_leaf(opts, snapshot_file),
    TreeSubcommand::Delete(opts) => handle_delete(opts, snapshot_file),
    TreeSubcommand::InsertBefore(opts) => handle_insert_before(opts, snapshot_file),
    TreeSubcommand::InsertAfter(opts) => handle_insert_after(opts, snapshot_file),
    TreeSubcommand::InsertChild(opts) => handle_insert_child(opts, snapshot_file),
    TreeSubcommand::AppendChild(opts) => handle_append_child(opts, snapshot_file),
    TreeSubcommand::SwapNext(opts) => handle_swap_next(opts, snapshot_file),
    TreeSubcommand::SwapPrev(opts) => handle_swap_prev(opts, snapshot_file),
    TreeSubcommand::Wrap(opts) => handle_wrap(opts, snapshot_file),
    TreeSubcommand::TargetReplace(opts) => handle_target_replace(opts, snapshot_file),
    TreeSubcommand::Cp(opts) => handle_cp(opts, snapshot_file),
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

/// Find the first leaf (preorder) and format for preview
fn first_leaf_preview(node: &Cirru) -> Option<String> {
  match node {
    Cirru::Leaf(s) => Some(format!("{:?}", s.as_ref())),
    Cirru::List(items) => items.iter().find_map(first_leaf_preview),
  }
}

/// Format a child node for list preview (leaf or list with first leaf)
fn format_child_preview(node: &Cirru) -> String {
  match node {
    Cirru::Leaf(s) => format!("{:?}", s.as_ref()),
    Cirru::List(items) => {
      if items.is_empty() {
        "(empty)".to_string()
      } else {
        let head = first_leaf_preview(node).unwrap_or_else(|| "<expr>".to_string());
        format!("({head} ...)")
      }
    }
  }
}

/// Show a side-by-side diff preview of the change
fn show_diff_preview(old_node: &Cirru, new_node: &Cirru, operation: &str, path: &[usize]) -> String {
  let mut output = String::new();

  output.push_str(&format!(
    "\n{}: {} at path [{}]\n",
    "Preview".blue().bold(),
    operation,
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
  ));
  output.push('\n');

  // Show old and new side by side (simplified version)
  let old_preview = format_preview_with_type(old_node, 10);
  let new_preview = format_preview_with_type(new_node, 10);

  output.push_str(&format!("{}:\n", "Before".yellow().bold()));
  output.push_str(&old_preview);
  output.push_str("\n\n");
  output.push_str(&format!("{}:\n", "After".green().bold()));
  output.push_str(&new_preview);
  output.push('\n');

  output
}

// ============================================================================
// Command handlers
// ============================================================================

fn handle_show(opts: &TreeShowCommand, snapshot_file: &str, show_json: bool) -> Result<(), String> {
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

  // Try to navigate to path, provide enhanced error message on failure
  let node = match navigate_to_path(&code_entry.code, &path) {
    Ok(n) => n,
    Err(original_error) => {
      // Find the longest valid path
      let mut valid_depth = 0;
      let mut current = &code_entry.code;

      for (depth, &idx) in path.iter().enumerate() {
        match current {
          Cirru::Leaf(_) => {
            valid_depth = depth;
            break;
          }
          Cirru::List(items) => {
            if idx >= items.len() {
              valid_depth = depth;
              break;
            }
            current = &items[idx];
            valid_depth = depth + 1;
          }
        }
      }

      // Get the node at the longest valid path
      let valid_path = &path[..valid_depth];
      let valid_node = navigate_to_path(&code_entry.code, valid_path).unwrap();

      // Format the valid path display
      let valid_path_display = if valid_path.is_empty() {
        "root".to_string()
      } else {
        format!("[{}]", valid_path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","))
      };

      // Get preview of the valid node
      let node_preview = match &valid_node {
        Cirru::Leaf(s) => format!("{:?} (leaf)", s.as_ref()),
        Cirru::List(items) => {
          let preview = valid_node.format_one_liner().unwrap_or_else(|_| "<complex>".to_string());
          let truncated = if preview.len() > 60 {
            format!("{}...", &preview[..60])
          } else {
            preview
          };
          format!("{} ({} items)", truncated, items.len())
        }
      };

      // Print enhanced error message
      eprintln!("{}", "Error: Invalid path".red().bold());
      eprintln!("{original_error}");
      eprintln!();
      eprintln!("{} Longest valid path: {}", "→".cyan(), valid_path_display.yellow());
      eprintln!("{} Node at that path: {}", "→".cyan(), node_preview.dimmed());
      eprintln!();

      // Show next steps based on node type
      match &valid_node {
        Cirru::Leaf(_) => {
          eprintln!("{} This is a leaf node (cannot navigate deeper)", "Note:".yellow().bold());
          eprintln!(
            "{} View it with: {}",
            "→".cyan(),
            format!(
              "cr tree show {} -p '{}'",
              opts.target,
              valid_path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
            )
            .cyan()
          );
        }
        Cirru::List(items) => {
          eprintln!(
            "{} This node has {} children (indices 0-{})",
            "Available:".green().bold(),
            items.len(),
            items.len().saturating_sub(1)
          );
          eprintln!(
            "{} View it with: {}",
            "→".cyan(),
            format!(
              "cr tree show {} -p '{}'",
              opts.target,
              valid_path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
            )
            .cyan()
          );

          // Show first few children as hints
          if !items.is_empty() {
            eprintln!();
            eprintln!("{} First few children:", "Hint:".blue().bold());
            for (i, item) in items.iter().enumerate().take(3) {
              let child_preview = format_child_preview(item);
              let child_path = if valid_path.is_empty() {
                i.to_string()
              } else {
                format!("{},{}", valid_path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","), i)
              };
              eprintln!("  [{}] {} {} -p '{}'", i, child_preview.yellow(), "->".dimmed(), child_path);
            }
            if items.len() > 3 {
              eprintln!("  {}", format!("... and {} more", items.len() - 3).dimmed());
            }
          }
        }
      }

      return Err(String::new()); // Empty error since we already printed detailed message
    }
  };

  // Print info
  let path_display = if path.is_empty() {
    "(root)".to_string()
  } else {
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
  };
  println!(
    "{}: {}  path: [{}]",
    "Location".green().bold(),
    format!("{namespace}/{definition}").cyan(),
    path_display
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
          let type_str = format_child_preview(item);
          let child_path = if opts.path.is_empty() {
            i.to_string()
          } else {
            format!("{},{}", opts.path, i)
          };
          println!("  [{}] {} {} -p '{}'", i, type_str.yellow(), "->".dimmed(), child_path);
        }
        println!();
      }

      if show_json {
        println!("{}:", "JSON".green().bold());
        println!("{}", cirru_to_json(&node));
        if opts.depth > 0 {
          println!("{}", format!("(depth limited to {})", opts.depth).dimmed());
        }
        println!();
      }

      println!("{}: To modify this node:", "Next steps".blue().bold());
      println!(
        "  • Replace: {} {} -p '{}' {}",
        "cr tree replace".cyan(),
        opts.target,
        opts.path,
        "-e 'cirru one-liner'".dimmed()
      );
      println!("  • Delete:  {} {} -p '{}'", "cr tree delete".cyan(), opts.target, opts.path);
      println!();
      let mut tips = Tips::new();
      tips.append(tip_prefer_oneliner_json(show_json));
      tips.print();

      return Ok(());
    }
  };

  if matches!(node_type, "leaf") {
    println!("{}: {}", "Type".green().bold(), "leaf".yellow());
    if let Cirru::Leaf(s) = &node {
      println!("{}: {:?}", "Value".green().bold(), s.as_ref());
      println!();
      println!("{}: To modify this leaf:", "Next steps".blue().bold());
      println!(
        "  • Replace: {} {} -p '{}' --leaf -e '<value>'",
        "cr tree replace".cyan(),
        opts.target,
        opts.path
      );
      if !path.is_empty() {
        // Show parent path for context
        let parent_path = &path[..path.len() - 1];
        let parent_path_str = parent_path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        println!(
          "  • View parent: {} {} -p '{}'",
          "cr tree show".cyan(),
          opts.target,
          parent_path_str
        );
      }
      println!();
      println!(
        "{}: Use {} for symbols, {} for strings",
        "Tip".blue().bold(),
        "-e 'symbol'".yellow(),
        "-e '|text'".yellow()
      );
    }
  }

  Ok(())
}

fn handle_replace(opts: &TreeReplaceCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;
  let path = parse_path(&opts.path)?;

  let code_input = read_code_input(&opts.file, &opts.code, &opts.json)?;

  let raw = code_input.as_deref().ok_or(ERR_CODE_INPUT_REQUIRED)?;

  let new_node = parse_input_to_cirru(raw, &opts.json, opts.json_input, opts.leaf, true)?;

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

  // Show diff preview
  println!("{}", show_diff_preview(&old_node, &processed_node, "replace", &path));
  // Tips: root-edit guidance
  if let Some(t) = tip_root_edit(path.is_empty()) {
    let mut tips = Tips::new();
    tips.add(t);
    tips.print();
  }

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
  println!();
  println!("{}", "Next steps:".blue().bold());
  println!(
    "  • Verify: {} '{}' -p '{}'",
    "cr tree show".cyan(),
    format_args!("{}/{}", namespace, definition),
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
  );
  println!("  • Check errors: {}", "cr query error".cyan());
  println!("  • Find usages: {} '{}/{}'", "cr query usages".cyan(), namespace, definition);

  Ok(())
}

fn handle_replace_leaf(opts: &TreeReplaceLeafCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let code_input = read_code_input(&opts.file, &opts.code, &opts.json)?;
  let raw = code_input.as_deref().ok_or(ERR_CODE_INPUT_REQUIRED)?;

  let replacement_node = parse_input_to_cirru(raw, &opts.json, opts.json_input, opts.leaf, true)?;

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

  // Find all leaf nodes that match the pattern
  let matches = find_all_leaf_matches(&code_entry.code, &opts.pattern, &[]);

  if matches.is_empty() {
    println!("{}", "No matches found.".yellow());
    return Ok(());
  }

  println!(
    "{} Found {} match(es) for pattern '{}' in '{}/{}':",
    "Search:".bold(),
    matches.len(),
    opts.pattern.yellow(),
    namespace,
    definition
  );
  println!();

  // Show preview of matches
  for (i, (path, old_value)) in matches.iter().enumerate().take(20) {
    let path_str = path.iter().map(|idx| idx.to_string()).collect::<Vec<_>>().join(",");
    println!("  {}. Path [{}]: {}", i + 1, path_str.dimmed(), format!("{old_value:?}").yellow());
  }

  if matches.len() > 20 {
    println!("  ... and {} more", matches.len() - 20);
  }
  println!();

  // Replace all matches (from end to beginning to maintain path validity)
  let mut new_code = code_entry.code.clone();
  let mut replaced_count = 0;

  // Sort paths in reverse order to replace from deepest/rightmost first
  let mut sorted_matches = matches.clone();
  sorted_matches.sort_by(|a, b| b.0.cmp(&a.0));

  for (path, _) in sorted_matches {
    match apply_operation_at_path(&new_code, &path, "replace", Some(&replacement_node)) {
      Ok(updated_code) => {
        new_code = updated_code;
        replaced_count += 1;
      }
      Err(e) => {
        eprintln!(
          "{} Failed to replace at path [{}]: {}",
          "Warning:".yellow(),
          path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
          e
        );
      }
    }
  }

  // Update the code entry
  code_entry.code = new_code;
  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Replaced {} occurrence(s) in '{}/{}'",
    "✓".green(),
    replaced_count,
    namespace,
    definition
  );
  println!();
  println!("{}:", "Replacement".green().bold());
  println!(
    "  {} → {}",
    format!("{:?}", opts.pattern).yellow(),
    format_preview_with_type(&replacement_node, 0)
  );
  println!();
  println!("{}", "Next steps:".blue().bold());
  println!("  • Verify: {} '{}/{}'", "cr query def".cyan(), namespace, definition);
  println!("  • Check errors: {}", "cr query error".cyan());
  println!("  • Find usages: {} '{}/{}'", "cr query usages".cyan(), namespace, definition);

  Ok(())
}

fn handle_target_replace(opts: &TreeTargetReplaceCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let code_input = read_code_input(&opts.file, &opts.code, &opts.json)?;
  let raw = code_input.as_deref().ok_or(ERR_CODE_INPUT_REQUIRED)?;

  let replacement_node = parse_input_to_cirru(raw, &opts.json, opts.json_input, opts.leaf, true)?;

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

  // Find all leaf nodes that match the pattern
  let matches = find_all_leaf_matches(&code_entry.code, &opts.pattern, &[]);

  if matches.is_empty() {
    return Err(format!(
      "No matches found for pattern '{}' in '{}/{}'",
      opts.pattern, namespace, definition
    ));
  }

  if matches.len() > 1 {
    println!(
      "{} Found {} matches for pattern '{}' in '{}/{}'.",
      "Notice:".yellow().bold(),
      matches.len(),
      opts.pattern.yellow(),
      namespace,
      definition
    );
    println!("Please use specific path to replace:");
    println!();

    let replacement_arg = if let Some(c) = &opts.code {
      format!("-e '{c}'")
    } else if let Some(j) = &opts.json {
      format!("-j '{j}'")
    } else if let Some(f) = &opts.file {
      format!("-f '{f}'")
    } else {
      "-e '...'".to_string()
    };

    for (i, (path, _)) in matches.iter().enumerate().take(10) {
      let path_str = path.iter().map(|idx| idx.to_string()).collect::<Vec<_>>().join(",");
      println!(
        "  {}. {} {} -p '{}' {}",
        i + 1,
        "cr tree replace".cyan(),
        opts.target,
        path_str,
        replacement_arg
      );
    }

    if matches.len() > 10 {
      println!("  ... and {} more", matches.len() - 10);
    }
    println!();
    println!("{}", "Tip: Use 'tree replace-leaf' if you want to replace ALL occurrences.".blue());

    return Err(String::new());
  }

  // Exactly one match
  let (path, old_value) = &matches[0];
  let old_node = Cirru::Leaf(old_value.to_string().into());

  // Process node with replacements if needed
  let processed_node = if opts.refer_original.is_some() || opts.refer_inner_branch.is_some() {
    process_node_with_references(
      &replacement_node,
      Some(&old_node),
      &opts.refer_original,
      &opts.refer_inner_branch,
      &opts.refer_inner_placeholder,
    )?
  } else {
    replacement_node
  };

  // Show diff preview
  println!("{}", show_diff_preview(&old_node, &processed_node, "target-replace", path));

  let new_code = apply_operation_at_path(&code_entry.code, path, "replace", Some(&processed_node))?;
  code_entry.code = new_code;

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Replaced unique occurrence in '{}/{}' at path [{}]",
    "✓".green(),
    namespace,
    definition,
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
  );

  Ok(())
}

/// Find all leaf nodes that exactly match the pattern
fn find_all_leaf_matches(node: &Cirru, pattern: &str, current_path: &[usize]) -> Vec<(Vec<usize>, String)> {
  let mut results = Vec::new();

  match node {
    Cirru::Leaf(s) => {
      // Exact match only
      if s.as_ref() == pattern {
        results.push((current_path.to_vec(), s.to_string()));
      }
    }
    Cirru::List(items) => {
      // Recursively search children
      for (i, item) in items.iter().enumerate() {
        let mut new_path = current_path.to_vec();
        new_path.push(i);
        results.extend(find_all_leaf_matches(item, pattern, &new_path));
      }
    }
  }

  results
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
  let old_parent = if parent_path.is_empty() {
    code_entry.code.clone()
  } else {
    navigate_to_path(&code_entry.code, &parent_path)?
  };

  // Show diff preview with parent context
  println!(
    "\n{}: Deleting node at path [{}]",
    "Preview".blue().bold(),
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
  );
  println!("{}:", "Node to delete".yellow().bold());
  println!("{}", format_preview_with_type(&old_node, 10));
  println!();
  println!("{}:", "Parent context".dimmed());
  println!("{}", format_preview_with_type(&old_parent, 8));
  println!();
  if let Some(t) = tip_root_edit(path.is_empty()) {
    let mut tips = Tips::new();
    tips.add(t);
    tips.print();
  }

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
  println!();

  // Warn about index changes
  if !path.is_empty() {
    let deleted_index = path[path.len() - 1];
    println!(
      "{}: Sibling nodes after index {} have shifted down by 1",
      "⚠️  Index change".yellow().bold(),
      deleted_index
    );
    println!(
      "   Example: path [{},{}] is now [{},{}]",
      parent_path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
      deleted_index + 1,
      parent_path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
      deleted_index
    );
    println!(
      "   {}: Re-run {} to get updated paths",
      "Tip".blue().bold(),
      "cr query search".cyan()
    );
  }

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
  fn json_input(&self) -> bool;
  fn leaf(&self) -> bool;
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
  fn json_input(&self) -> bool {
    self.json_input
  }
  fn leaf(&self) -> bool {
    self.leaf
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
  fn json_input(&self) -> bool {
    self.json_input
  }
  fn leaf(&self) -> bool {
    self.leaf
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
  fn json_input(&self) -> bool {
    self.json_input
  }
  fn leaf(&self) -> bool {
    self.leaf
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
  fn json_input(&self) -> bool {
    self.json_input
  }
  fn leaf(&self) -> bool {
    self.leaf
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
  fn json_input(&self) -> bool {
    self.json_input
  }
  fn leaf(&self) -> bool {
    self.leaf
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

impl InsertOperation for TreeTargetReplaceCommand {
  fn file(&self) -> &Option<String> {
    &self.file
  }
  fn code(&self) -> &Option<String> {
    &self.code
  }
  fn json(&self) -> &Option<String> {
    &self.json
  }
  fn json_input(&self) -> bool {
    self.json_input
  }
  fn leaf(&self) -> bool {
    self.leaf
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

  let code_input = read_code_input(opts.file(), opts.code(), opts.json())?;

  let raw = code_input.as_deref().ok_or(ERR_CODE_INPUT_REQUIRED)?;

  let new_node = parse_input_to_cirru(raw, opts.json(), opts.json_input(), opts.leaf(), true)?;

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

  // Show diff preview
  println!(
    "\n{}: {} at path [{}]",
    "Preview".blue().bold(),
    operation,
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
  );
  println!("{}:", "Node to insert".cyan().bold());
  println!("{}", format_preview_with_type(&processed_node, 8));
  println!();
  println!("{}:", "Parent before".dimmed());
  println!("{}", format_preview_with_type(&old_parent, 8));
  println!();
  if let Some(t) = tip_root_edit(path.is_empty()) {
    let mut tips = Tips::new();
    tips.add(t);
    tips.print();
  }

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
  println!();

  // Explain index impact based on operation
  match operation {
    "insert-before" => {
      if !path.is_empty() {
        let insert_index = path[path.len() - 1];
        println!(
          "{}: Node inserted at index {}, original node and siblings shifted up by 1",
          "Index impact".yellow().bold(),
          insert_index
        );
        println!(
          "   Old path [{},{}] → New path [{},{}]",
          parent_path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
          insert_index,
          parent_path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
          insert_index + 1
        );
      }
    }
    "insert-after" => {
      if !path.is_empty() {
        let ref_index = path[path.len() - 1];
        println!(
          "{}: Node inserted at index {}, nodes after reference shifted up by 1",
          "Index impact".yellow().bold(),
          ref_index + 1
        );
      }
    }
    "insert-child" => {
      println!(
        "{}: Node inserted as first child (index 0), all existing children shifted up by 1",
        "Index impact".yellow().bold()
      );
      println!("   Old child [0] → New child [1], [1] → [2], etc.");
    }
    "append-child" => {
      println!(
        "{}: Node appended as last child, no index changes to existing nodes",
        "Index impact".green().bold()
      );
      println!("   {}:  Use this for multiple insertions to keep paths stable", "Tip".blue().bold());
    }
    _ => {}
  }

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

  // Explain what was swapped
  if !path.is_empty() {
    let current_index = path[path.len() - 1];
    let parent_display = if parent_path.is_empty() {
      "root".to_string()
    } else {
      format!("[{}]", parent_path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","))
    };

    match operation {
      "swap-next-sibling" => {
        println!(
          "{}: Swapped child [{}] with [{}] under parent {}",
          "Index change".yellow().bold(),
          current_index,
          current_index + 1,
          parent_display
        );
      }
      "swap-prev-sibling" => {
        println!(
          "{}: Swapped child [{}] with [{}] under parent {}",
          "Index change".yellow().bold(),
          current_index,
          current_index - 1,
          parent_display
        );
      }
      _ => {}
    }
    println!();
  }

  println!("{}:", "Parent before swap".yellow().bold());
  println!("{}", format_preview_with_type(&old_parent, 15));
  println!();
  if let Some(t) = tip_root_edit(path.is_empty()) {
    let mut tips = Tips::new();
    tips.add(t);
    tips.print();
  }
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

fn handle_cp(opts: &TreeCpCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;
  let from_path = parse_path(&opts.from)?;
  let to_path = parse_path(&opts.path)?;

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

  let source_node = navigate_to_path(&code_entry.code, &from_path)?.clone();

  let operation = match opts.at.as_str() {
    "before" => "insert-before",
    "after" => "insert-after",
    "prepend-child" => "insert-child",
    "append-child" => "append-child",
    "replace" => "replace",
    other => {
      return Err(format!(
        "Unsupported position '{other}'. Use: before, after, prepend-child, append-child, replace"
      ));
    }
  };

  let new_code = apply_operation_at_path(&code_entry.code, &to_path, operation, Some(&source_node))?;
  code_entry.code = new_code;

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Copied node from [{}] to [{}] ({}) in '{}/{}'",
    "✓".green(),
    opts.from,
    opts.path,
    opts.at,
    namespace,
    definition
  );

  Ok(())
}
