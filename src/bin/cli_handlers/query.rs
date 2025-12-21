//! Query subcommand handlers
//!
//! Handles: cr query ls-ns, ls-defs, read-ns, read-def, read-at, peek-def, find-symbol, usages, pkg-name, configs, error, ls-modules

use calcit::cli_args::{QueryCommand, QuerySubcommand};
use calcit::snapshot;
use calcit::util::string::strip_shebang;
use cirru_parser::Cirru;
use colored::Colorize;
use std::fs;
use std::path::Path;

pub fn handle_query_command(cmd: &QueryCommand, input_path: &str) -> Result<(), String> {
  match &cmd.subcommand {
    QuerySubcommand::LsNs(opts) => handle_ls_ns(input_path, opts.deps),
    QuerySubcommand::LsDefs(opts) => handle_ls_defs(input_path, &opts.namespace),
    QuerySubcommand::ReadNs(opts) => handle_read_ns(input_path, &opts.namespace),
    QuerySubcommand::PkgName(_) => handle_pkg_name(input_path),
    QuerySubcommand::Configs(_) => handle_configs(input_path),
    QuerySubcommand::Error(_) => handle_error(),
    QuerySubcommand::LsModules(_) => handle_ls_modules(input_path),
    QuerySubcommand::ReadDef(opts) => handle_read_def(input_path, &opts.namespace, &opts.definition),
    QuerySubcommand::ReadAt(opts) => handle_read_at(input_path, &opts.namespace, &opts.definition, &opts.path, opts.depth),
    QuerySubcommand::PeekDef(opts) => handle_peek_def(input_path, &opts.namespace, &opts.definition),
    QuerySubcommand::FindSymbol(opts) => handle_find_symbol(input_path, &opts.symbol, opts.deps),
    QuerySubcommand::Usages(opts) => handle_usages(input_path, &opts.namespace, &opts.definition, opts.deps),
  }
}

fn load_snapshot(input_path: &str) -> Result<snapshot::Snapshot, String> {
  if !Path::new(input_path).exists() {
    return Err(format!("{input_path} does not exist"));
  }

  let mut content = fs::read_to_string(input_path).map_err(|e| format!("Failed to read file: {e}"))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content)?;
  snapshot::load_snapshot_data(&data, input_path)
}

fn handle_ls_ns(input_path: &str, include_deps: bool) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  println!("{}", "Namespaces in project:".bold());
  let mut namespaces: Vec<&String> = snapshot.files.keys().collect();
  namespaces.sort();

  for ns in &namespaces {
    // Skip core namespaces unless deps is requested
    if !include_deps && (ns.starts_with("calcit.") || ns.starts_with("calcit-test.")) {
      continue;
    }
    println!("  {}", ns.cyan());
  }

  if include_deps {
    println!("\n{}", "(includes core/dependency namespaces)".dimmed());
  }

  Ok(())
}

fn handle_ls_defs(input_path: &str, namespace: &str) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  println!("{} {}", "Definitions in".bold(), namespace.cyan());

  let mut defs: Vec<&String> = file_data.defs.keys().collect();
  defs.sort();

  for def in &defs {
    let entry = &file_data.defs[*def];
    if !entry.doc.is_empty() {
      println!("  {} - {}", def.green(), entry.doc.dimmed());
    } else {
      println!("  {}", def.green());
    }
  }

  println!("\n{} {} definitions", "Total:".dimmed(), defs.len());
  Ok(())
}

fn handle_read_ns(input_path: &str, namespace: &str) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  println!("{} {}", "Namespace:".bold(), namespace.cyan());

  if !file_data.ns.doc.is_empty() {
    println!("{} {}", "Doc:".bold(), file_data.ns.doc);
  }

  // Print ns declaration (which includes import rules)
  println!("\n{}", "NS declaration:".bold());
  let ns_str = cirru_parser::format(&[file_data.ns.code.clone()], true.into()).unwrap_or_else(|_| "(failed to format)".to_string());
  println!("{}", ns_str.dimmed());

  // Print definitions with preview
  println!("\n{}", "Definitions:".bold());
  let mut defs: Vec<(&String, &snapshot::CodeEntry)> = file_data.defs.iter().collect();
  defs.sort_by_key(|(name, _)| *name);

  for (def_name, code_entry) in defs {
    let code_str = cirru_parser::format(&[code_entry.code.clone()], true.into()).unwrap_or_else(|_| "(failed)".to_string());
    let code_preview = if code_str.len() > 60 {
      format!("{}...", &code_str[..60])
    } else {
      code_str
    };

    if !code_entry.doc.is_empty() {
      println!("  {} - {}", def_name.green(), code_entry.doc.dimmed());
    } else {
      println!("  {}", def_name.green());
    }
    println!("    {}", code_preview.dimmed());
  }

  Ok(())
}

fn handle_pkg_name(input_path: &str) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;
  println!("{}", snapshot.package);
  Ok(())
}

fn handle_configs(input_path: &str) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  println!("{}", "Project Configs:".bold());
  println!("  {}: {}", "init_fn".cyan(), snapshot.configs.init_fn);
  println!("  {}: {}", "reload_fn".cyan(), snapshot.configs.reload_fn);
  println!("  {}: {}", "version".cyan(), snapshot.configs.version);
  println!("  {}: {:?}", "modules".cyan(), snapshot.configs.modules);

  Ok(())
}

fn handle_error() -> Result<(), String> {
  let error_file = ".calcit-error.cirru";

  if !Path::new(error_file).exists() {
    println!("{}", "No .calcit-error.cirru file found.".yellow());
    return Ok(());
  }

  let content = fs::read_to_string(error_file).map_err(|e| format!("Failed to read error file: {e}"))?;

  if content.trim().is_empty() {
    println!("{}", "Error file is empty (no recent errors).".green());
  } else {
    println!("{}", "Last error stack trace:".bold().red());
    println!("{content}");
  }

  Ok(())
}

fn handle_ls_modules(input_path: &str) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  println!("{}", "Modules in project:".bold());

  // Print main package
  println!("  {} (main)", snapshot.package.cyan());

  // Print config entries (modules)
  if !snapshot.configs.modules.is_empty() {
    println!("\n{}", "Dependencies:".bold());
    for module in &snapshot.configs.modules {
      println!("  {}", module.cyan());
    }
  }

  // Print entries if any
  if !snapshot.entries.is_empty() {
    println!("\n{}", "Entries:".bold());
    for name in snapshot.entries.keys() {
      println!("  {}", name.cyan());
    }
  }

  Ok(())
}

fn handle_read_def(input_path: &str, namespace: &str, definition: &str) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found in namespace '{namespace}'"))?;

  // Output as JSON for machine consumption
  let json = cirru_to_json(&code_entry.code);
  println!("{}", serde_json::to_string_pretty(&json).unwrap());

  Ok(())
}

fn handle_read_at(input_path: &str, namespace: &str, definition: &str, path: &str, max_depth: usize) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found in namespace '{namespace}'"))?;

  // Parse path
  let indices: Vec<usize> = if path.is_empty() {
    vec![]
  } else {
    path
      .split(',')
      .map(|s| s.trim().parse::<usize>())
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| format!("Invalid path format: {e}"))?
  };

  // Navigate to target
  let target = navigate_to_path(&code_entry.code, &indices)?;

  // Output info
  println!("{} {}/{}", "Reading:".bold(), namespace.cyan(), definition.green());
  println!("{} [{}]", "Path:".bold(), path);

  // Show target type and length if it's a list
  match &target {
    Cirru::Leaf(s) => {
      println!("{} leaf", "Type:".bold());
      println!("{} {}", "Value:".bold(), s.to_string().yellow());
    }
    Cirru::List(items) => {
      println!("{} list ({} items)", "Type:".bold(), items.len());
      // Show children summary
      println!("\n{}", "Children:".bold());
      for (i, item) in items.iter().enumerate() {
        let summary = match item {
          Cirru::Leaf(s) => format!("leaf: {s}"),
          Cirru::List(sub_items) => format!("list ({} items)", sub_items.len()),
        };
        println!("  [{}] {}", i.to_string().dimmed(), summary);
      }
    }
  }

  // Also output JSON for programmatic use (with depth limit)
  println!("\n{}", "JSON:".bold());
  let json = cirru_to_json_with_depth(&target, max_depth, 0);
  println!("{}", serde_json::to_string_pretty(&json).unwrap());
  if max_depth > 0 {
    println!("{}", format!("(depth limited to {max_depth})").dimmed());
  }

  Ok(())
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

fn cirru_to_json(cirru: &Cirru) -> serde_json::Value {
  match cirru {
    Cirru::Leaf(s) => serde_json::Value::String(s.to_string()),
    Cirru::List(items) => serde_json::Value::Array(items.iter().map(cirru_to_json).collect()),
  }
}

/// Convert Cirru to JSON with depth limit (0 = unlimited)
pub fn cirru_to_json_with_depth(cirru: &Cirru, max_depth: usize, current_depth: usize) -> serde_json::Value {
  match cirru {
    Cirru::Leaf(s) => serde_json::Value::String(s.to_string()),
    Cirru::List(items) => {
      if max_depth > 0 && current_depth >= max_depth {
        // At max depth, show truncated indicator
        if items.is_empty() {
          serde_json::Value::Array(vec![])
        } else {
          // Show first item (usually the operator) and indicate more items
          let first = match &items[0] {
            Cirru::Leaf(s) => serde_json::Value::String(s.to_string()),
            Cirru::List(_) => serde_json::Value::String("[...]".to_string()),
          };
          if items.len() == 1 {
            serde_json::Value::Array(vec![first])
          } else {
            serde_json::Value::Array(vec![first, serde_json::Value::String(format!("...{} more", items.len() - 1))])
          }
        }
      } else {
        serde_json::Value::Array(
          items
            .iter()
            .map(|item| cirru_to_json_with_depth(item, max_depth, current_depth + 1))
            .collect(),
        )
      }
    }
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Progressive disclosure commands
// ═══════════════════════════════════════════════════════════════════════════════

/// Peek definition - show signature/params/doc without full body (Level 2 disclosure)
fn handle_peek_def(input_path: &str, namespace: &str, definition: &str) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found in namespace '{namespace}'"))?;

  println!("{} {}/{}", "Definition:".bold(), namespace.cyan(), definition.green());

  // Show doc if present
  if !code_entry.doc.is_empty() {
    println!("{} {}", "Doc:".bold(), code_entry.doc);
  }

  // Extract signature info from the code
  match &code_entry.code {
    Cirru::List(items) if !items.is_empty() => {
      // Get the form type (defn, defmacro, def, etc.)
      let form_type = match &items[0] {
        Cirru::Leaf(s) => s.to_string(),
        _ => "unknown".to_string(),
      };
      println!("{} {}", "Form:".bold(), form_type.yellow());

      // For defn/defmacro, extract params
      if (form_type == "defn" || form_type == "defmacro" || form_type == "defn-") && items.len() >= 3 {
        // items[1] = name, items[2] = params
        if let Cirru::List(params) = &items[2] {
          let param_names: Vec<String> = params
            .iter()
            .map(|p| match p {
              Cirru::Leaf(s) => s.to_string(),
              Cirru::List(_) => "[...]".to_string(),
            })
            .collect();
          println!("{} ({})", "Params:".bold(), param_names.join(" "));
        }

        // Show body count (how many expressions in body)
        let body_count = items.len() - 3;
        println!("{} {} expression(s)", "Body:".bold(), body_count);

        // Show first expression head for context (depth limited)
        if items.len() > 3 {
          let first_body = &items[3];
          let preview = cirru_to_json_with_depth(first_body, 1, 0);
          println!("{} {}", "First expr:".bold(), serde_json::to_string(&preview).unwrap().dimmed());
        }
      } else if form_type == "def" && items.len() >= 3 {
        // For def, show value preview
        let value = &items[2];
        let preview = cirru_to_json_with_depth(value, 1, 0);
        println!("{} {}", "Value:".bold(), serde_json::to_string(&preview).unwrap().dimmed());
      }
    }
    _ => {
      println!("{}", "(empty or invalid definition)".dimmed());
    }
  }

  Ok(())
}

/// Find symbol across all namespaces
fn handle_find_symbol(input_path: &str, symbol: &str, include_deps: bool) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  println!("{} '{}'\n", "Searching for symbol:".bold(), symbol.yellow());

  let mut found_definitions: Vec<(String, String)> = vec![];
  let mut found_references: Vec<(String, String, String)> = vec![]; // (ns, def, context)

  for (ns_name, file_data) in &snapshot.files {
    // Skip core namespaces unless deps is requested
    if !include_deps && (ns_name.starts_with("calcit.") || ns_name.starts_with("calcit-test.")) {
      continue;
    }

    // Check if symbol is defined in this namespace
    if file_data.defs.contains_key(symbol) {
      found_definitions.push((ns_name.clone(), symbol.to_string()));
    }

    // Search for references in all definitions
    for (def_name, code_entry) in &file_data.defs {
      if find_symbol_in_cirru(&code_entry.code, symbol, def_name != symbol) {
        found_references.push((ns_name.clone(), def_name.clone(), get_symbol_context(&code_entry.code, symbol)));
      }
    }
  }

  // Print definitions
  if !found_definitions.is_empty() {
    println!("{}", "Definitions:".bold().green());
    for (ns, def) in &found_definitions {
      println!("  {}/{}", ns.cyan(), def.green());
    }
    println!();
  }

  // Print references (excluding the definition itself)
  let references: Vec<_> = found_references
    .iter()
    .filter(|(ns, def, _)| !found_definitions.iter().any(|(dns, ddef)| dns == ns && ddef == def))
    .collect();

  if !references.is_empty() {
    println!("{}", "Referenced in:".bold());
    for (ns, def, context) in &references {
      println!("  {}/{}", ns.cyan(), def);
      if !context.is_empty() {
        println!("    {}", context.dimmed());
      }
    }
  }

  if found_definitions.is_empty() && references.is_empty() {
    println!("{}", "No matches found.".yellow());
  }

  Ok(())
}

/// Find usages of a specific definition
fn handle_usages(input_path: &str, target_ns: &str, target_def: &str, include_deps: bool) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  // Verify the target definition exists
  let _ = snapshot
    .files
    .get(target_ns)
    .ok_or_else(|| format!("Namespace '{target_ns}' not found"))?
    .defs
    .get(target_def)
    .ok_or_else(|| format!("Definition '{target_def}' not found in namespace '{target_ns}'"))?;

  println!("{} {}/{}\n", "Finding usages of:".bold(), target_ns.cyan(), target_def.green());

  let mut usages: Vec<(String, String, String)> = vec![]; // (ns, def, context)

  for (ns_name, file_data) in &snapshot.files {
    // Skip core namespaces unless deps is requested
    if !include_deps && (ns_name.starts_with("calcit.") || ns_name.starts_with("calcit-test.")) {
      continue;
    }

    // Check if this namespace imports from target_ns
    let imports_target = check_ns_imports(&file_data.ns.code, target_ns, target_def);

    for (def_name, code_entry) in &file_data.defs {
      // Skip the definition itself
      if ns_name == target_ns && def_name == target_def {
        continue;
      }

      // Search for the symbol (could be qualified or unqualified depending on import)
      let found = if imports_target || ns_name == target_ns {
        find_symbol_in_cirru(&code_entry.code, target_def, true)
      } else {
        // Check for qualified reference: target_ns/target_def
        let qualified = format!("{target_ns}/{target_def}");
        find_symbol_in_cirru(&code_entry.code, &qualified, true)
      };

      if found {
        let context = get_symbol_context(&code_entry.code, target_def);
        usages.push((ns_name.clone(), def_name.clone(), context));
      }
    }
  }

  if usages.is_empty() {
    println!("{}", "No usages found.".yellow());
  } else {
    println!("{} {} usage(s):\n", "Found".bold(), usages.len());
    for (ns, def, context) in &usages {
      println!("  {}/{}", ns.cyan(), def.green());
      if !context.is_empty() {
        println!("    {}", context.dimmed());
      }
    }
  }

  Ok(())
}

// Helper: recursively search for symbol in Cirru tree
fn find_symbol_in_cirru(code: &Cirru, symbol: &str, skip_first: bool) -> bool {
  match code {
    Cirru::Leaf(s) => s.as_ref() == symbol,
    Cirru::List(items) => {
      let start = if skip_first { 1 } else { 0 };
      items.iter().skip(start).any(|item| find_symbol_in_cirru(item, symbol, false))
    }
  }
}

// Helper: get context around symbol usage (first expression containing it)
fn get_symbol_context(code: &Cirru, symbol: &str) -> String {
  match code {
    Cirru::Leaf(s) if s.as_ref() == symbol => symbol.to_string(),
    Cirru::List(items) => {
      for item in items {
        if find_symbol_in_cirru(item, symbol, false) {
          // Found it in this subtree, return a preview
          let preview = cirru_to_json_with_depth(item, 1, 0);
          return serde_json::to_string(&preview).unwrap_or_default();
        }
      }
      String::new()
    }
    _ => String::new(),
  }
}

// Helper: check if namespace imports the target
fn check_ns_imports(ns_code: &Cirru, target_ns: &str, _target_def: &str) -> bool {
  // ns_code is like (ns my-ns (:require [target-ns ...]))
  // Simplified check: just see if target_ns appears in the ns declaration
  match ns_code {
    Cirru::Leaf(s) => s.as_ref() == target_ns,
    Cirru::List(items) => items.iter().any(|item| check_ns_imports(item, target_ns, _target_def)),
  }
}
