//! Query subcommand handlers
//!
//! Handles: cr query ls-ns, ls-defs, read-ns, read-def, read-at, peek-def, find-symbol, usages, pkg-name, configs, error, ls-modules

use calcit::cli_args::{QueryCommand, QuerySubcommand};
use calcit::load_core_snapshot;
use calcit::snapshot;
use calcit::util::string::strip_shebang;
use cirru_parser::Cirru;
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Parse "namespace/definition" format into (namespace, definition)
fn parse_target(target: &str) -> Result<(&str, &str), String> {
  target
    .rsplit_once('/')
    .ok_or_else(|| format!("Invalid target format: '{target}'. Expected 'namespace/definition' (e.g. 'app.core/main')"))
}

pub fn handle_query_command(cmd: &QueryCommand, input_path: &str) -> Result<(), String> {
  match &cmd.subcommand {
    QuerySubcommand::LsNs(opts) => handle_ls_ns(input_path, opts.deps, opts.prefix.as_deref()),
    QuerySubcommand::LsDefs(opts) => handle_ls_defs(input_path, &opts.namespace),
    QuerySubcommand::ReadNs(opts) => handle_read_ns(input_path, &opts.namespace),
    QuerySubcommand::PkgName(_) => handle_pkg_name(input_path),
    QuerySubcommand::Configs(_) => handle_configs(input_path),
    QuerySubcommand::Error(_) => handle_error(),
    QuerySubcommand::LsModules(_) => handle_ls_modules(input_path),
    QuerySubcommand::ReadDef(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_read_def(input_path, ns, def)
    }
    QuerySubcommand::ReadAt(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_read_at(input_path, ns, def, &opts.path, opts.depth)
    }
    QuerySubcommand::PeekDef(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_peek_def(input_path, ns, def)
    }
    QuerySubcommand::ReadExamples(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_read_examples(input_path, ns, def)
    }
    QuerySubcommand::FindSymbol(opts) => {
      if opts.fuzzy {
        handle_fuzzy_search(input_path, &opts.symbol, opts.deps, opts.limit)
      } else {
        handle_find_symbol(input_path, &opts.symbol, opts.deps)
      }
    }
    QuerySubcommand::Usages(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_usages(input_path, ns, def, opts.deps)
    }
  }
}

/// Load a module silently (without println)
fn load_module_silent(path: &str, base_dir: &Path, module_folder: &Path) -> Result<snapshot::Snapshot, String> {
  let mut file_path = String::from(path);
  if file_path.ends_with('/') {
    file_path.push_str("compact.cirru");
  }

  let fullpath = if file_path.starts_with("./") {
    base_dir.join(&file_path).to_owned()
  } else if file_path.starts_with('/') {
    Path::new(&file_path).to_owned()
  } else {
    module_folder.join(&file_path).to_owned()
  };

  let mut content = fs::read_to_string(&fullpath).map_err(|e| format!("Failed to read {}: {}", fullpath.display(), e))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content)?;
  snapshot::load_snapshot_data(&data, &fullpath.display().to_string())
}

fn load_snapshot(input_path: &str) -> Result<snapshot::Snapshot, String> {
  if !Path::new(input_path).exists() {
    return Err(format!("{input_path} does not exist"));
  }

  let mut content = fs::read_to_string(input_path).map_err(|e| format!("Failed to read file: {e}"))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content)?;
  let mut snapshot = snapshot::load_snapshot_data(&data, input_path)?;

  // Load modules (dependencies) silently
  let base_dir = Path::new(input_path).parent().unwrap_or(Path::new("."));
  let module_folder = dirs::home_dir()
    .map(|buf| buf.as_path().join(".config/calcit/modules/"))
    .unwrap_or_else(|| Path::new(".").to_owned());

  for module_path in &snapshot.configs.modules.clone() {
    match load_module_silent(module_path, base_dir, &module_folder) {
      Ok(module_snapshot) => {
        for (ns_name, file_data) in module_snapshot.files {
          snapshot.files.entry(ns_name).or_insert(file_data);
        }
      }
      Err(e) => {
        eprintln!("Warning: Failed to load module '{module_path}': {e}");
      }
    }
  }

  // Merge calcit.core definitions from built-in calcit-core.cirru
  let core_snapshot = load_core_snapshot()?;
  for (ns_name, file_data) in core_snapshot.files {
    snapshot.files.entry(ns_name).or_insert(file_data);
  }

  Ok(snapshot)
}

fn handle_ls_ns(input_path: &str, include_deps: bool, prefix: Option<&str>) -> Result<(), String> {
  // Load snapshot without deps first to identify main package namespaces
  if !Path::new(input_path).exists() {
    return Err(format!("{input_path} does not exist"));
  }

  let mut content = fs::read_to_string(input_path).map_err(|e| format!("Failed to read file: {e}"))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content)?;
  let main_snapshot = snapshot::load_snapshot_data(&data, input_path)?;
  let main_package = main_snapshot.package.clone();

  // Now load full snapshot with deps if needed
  let snapshot = if include_deps { load_snapshot(input_path)? } else { main_snapshot };

  let mut namespaces: Vec<&String> = snapshot.files.keys().collect();
  namespaces.sort();

  // Filter by:
  // 1. If prefix is set, filter by prefix
  // 2. If --deps is not set, only show namespaces from main package
  // 3. If --deps is set, show all including deps and core
  let filtered: Vec<_> = namespaces
    .iter()
    .filter(|ns| {
      // Apply prefix filter if set
      if let Some(p) = prefix {
        if !ns.starts_with(p) {
          return false;
        }
      }
      // If deps not included, only show main package namespaces
      if !include_deps {
        // Main package namespaces typically start with package name
        // e.g. package "respo" -> namespaces "respo.*"
        ns.as_str() == main_package || ns.starts_with(&format!("{main_package}."))
      } else {
        true
      }
    })
    .collect();

  if let Some(p) = prefix {
    println!(
      "{} ({} namespaces matching \"{}\")",
      if include_deps { "All namespaces:" } else { "Project namespaces:" }.bold(),
      filtered.len(),
      p.yellow()
    );
  } else {
    println!(
      "{} ({} namespaces)",
      if include_deps { "All namespaces:" } else { "Project namespaces:" }.bold(),
      filtered.len()
    );
  }

  for ns in &filtered {
    println!("  {}", ns.cyan());
  }

  // LLM guidance
  if !include_deps {
    println!("\n{}", "Tip: Use `--deps` to include dependency and core namespaces.".dimmed());
  }
  println!(
    "{}",
    "Tip: Use `query ls-defs <namespace>` to list definitions in a namespace.".dimmed()
  );

  Ok(())
}

fn handle_ls_defs(input_path: &str, namespace: &str) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let mut defs: Vec<&String> = file_data.defs.keys().collect();
  defs.sort();

  println!("{} {} ({} definitions)", "Namespace:".bold(), namespace.cyan(), defs.len());

  for def in &defs {
    let entry = &file_data.defs[*def];
    if !entry.doc.is_empty() {
      println!("  {} - {}", def.green(), entry.doc.dimmed());
    } else {
      println!("  {}", def.green());
    }
  }

  // LLM guidance
  println!(
    "\n{}",
    "Tip: Use `query peek-def <ns/def>` to see signature, or `query read-def <ns/def>` for full code.".dimmed()
  );

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
  let mut defs: Vec<(&String, &snapshot::CodeEntry)> = file_data.defs.iter().collect();
  defs.sort_by_key(|(name, _)| *name);

  println!("\n{} ({} total)", "Definitions:".bold(), defs.len());
  for (def_name, code_entry) in defs {
    let code_str = cirru_parser::format(&[code_entry.code.clone()], true.into()).unwrap_or_else(|_| "(failed)".to_string());
    // Take first non-empty line for preview
    let first_line = code_str.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    let preview = if first_line.len() > 70 {
      format!("{}...", &first_line[..70])
    } else {
      first_line.to_string()
    };

    if !code_entry.doc.is_empty() {
      println!("  {} - {}  {}", def_name.green(), code_entry.doc.dimmed(), preview.dimmed());
    } else {
      println!("  {}  {}", def_name.green(), preview.dimmed());
    }
  }

  // LLM guidance
  println!("\n{}", "Tip: Use `query peek-def <ns/def>` for signature details.".dimmed());

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
  if !Path::new(input_path).exists() {
    return Err(format!("{input_path} does not exist"));
  }

  let mut content = fs::read_to_string(input_path).map_err(|e| format!("Failed to read file: {e}"))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content)?;
  let snapshot = snapshot::load_snapshot_data(&data, input_path)?;

  let base_dir = Path::new(input_path).parent().unwrap_or(Path::new("."));
  let module_folder = dirs::home_dir()
    .map(|buf| buf.as_path().join(".config/calcit/modules/"))
    .unwrap_or_else(|| Path::new(".").to_owned());

  println!("{}", "Modules in project:".bold());

  // Print main package
  println!("  {} {}", snapshot.package.cyan(), "(main)".dimmed());

  // Print each dependency module with its package name
  for module_path in &snapshot.configs.modules {
    match load_module_silent(module_path, base_dir, &module_folder) {
      Ok(module_snapshot) => {
        println!("  {} {}", module_snapshot.package.cyan(), format!("({module_path})").dimmed());
      }
      Err(_) => {
        println!("  {} {}", module_path.yellow(), "(failed)".red());
      }
    }
  }

  // Print entries if any
  if !snapshot.entries.is_empty() {
    println!("\n{}", "Entries:".bold());
    for name in snapshot.entries.keys() {
      println!("  {}", name.cyan());
    }
  }

  // LLM guidance
  println!(
    "\n{}",
    "Tip: Use `query ls-ns` to list all namespaces, or `query ls-defs <namespace>` to list definitions.".dimmed()
  );

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

  // Output header
  println!("{} {}/{}", "Definition:".bold(), namespace.cyan(), definition.green());

  if !code_entry.doc.is_empty() {
    println!("{} {}", "Doc:".bold(), code_entry.doc);
  }

  // Output examples if present
  if !code_entry.examples.is_empty() {
    println!("\n{} ({} total)", "Examples:".bold(), code_entry.examples.len());
    for (i, example) in code_entry.examples.iter().enumerate() {
      let example_str = cirru_parser::format(&[example.clone()], true.into()).unwrap_or_else(|_| "(failed to format)".to_string());
      // Indent and truncate if too long
      let lines: Vec<&str> = example_str.lines().collect();
      if lines.len() <= 3 {
        println!("  {}:", format!("[{}]", i + 1).dimmed());
        for line in &lines {
          println!("    {line}");
        }
      } else {
        println!("  {}:", format!("[{}]", i + 1).dimmed());
        for line in lines.iter().take(3) {
          println!("    {line}");
        }
        println!("    {}", "...".dimmed());
      }
    }
  }

  // Output as Cirru format (human readable)
  println!("\n{}", "Cirru:".bold());
  let cirru_str = cirru_parser::format(&[code_entry.code.clone()], true.into()).unwrap_or_else(|_| "(failed to format)".to_string());
  println!("{cirru_str}");

  // Also output compact JSON for edit operations
  println!("\n{}", "JSON (for edit):".bold());
  let json = cirru_to_json(&code_entry.code);
  println!("{}", serde_json::to_string(&json).unwrap());

  // LLM guidance
  println!(
    "\n{}",
    format!("Tip: Use `query read-at {namespace}/{definition} -p \"0\"` to explore tree structure for editing.").dimmed()
  );

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

  // Output info - compact header
  println!(
    "{} {}/{}  {}",
    "At:".bold(),
    namespace.cyan(),
    definition.green(),
    format!("[{path}]").dimmed()
  );

  // Show target type and content
  match &target {
    Cirru::Leaf(s) => {
      println!("{} leaf = {}", "Type:".bold(), s.to_string().yellow());
      // For leaf, show JSON directly
      println!("{} \"{}\"", "JSON:".bold(), s);
    }
    Cirru::List(items) => {
      println!("{} list ({} items)", "Type:".bold(), items.len());

      // Show Cirru format preview (depth limited by formatting)
      let cirru_str = cirru_parser::format(&[target.clone()], true.into()).unwrap_or_else(|_| "(failed)".to_string());
      let preview_lines: Vec<&str> = cirru_str.lines().take(5).collect();
      println!("\n{}", "Cirru preview:".bold());
      for line in &preview_lines {
        println!("  {line}");
      }
      if cirru_str.lines().count() > 5 {
        println!("  {}", "...".dimmed());
      }

      // Show children index for navigation
      println!("\n{}", "Children:".bold());
      for (i, item) in items.iter().enumerate() {
        let child_path = if path.is_empty() { i.to_string() } else { format!("{path},{i}") };
        let summary = match item {
          Cirru::Leaf(s) => {
            let s_str = s.to_string();
            if s_str.len() > 30 {
              format!("\"{}...\"", &s_str[..30])
            } else {
              format!("\"{s_str}\"")
            }
          }
          Cirru::List(sub_items) => format!("({} items)", sub_items.len()),
        };
        println!("  [{i}] {summary} -> -p \"{child_path}\"");
      }

      // Output JSON for programmatic use
      println!("\n{}", "JSON:".bold());
      let json = cirru_to_json_with_depth(&target, max_depth, 0);
      println!("{}", serde_json::to_string(&json).unwrap());
      if max_depth > 0 {
        println!("{}", format!("(depth limited to {max_depth})").dimmed());
      }
    }
  }

  // LLM guidance based on context
  if path.is_empty() {
    println!(
      "\n{}",
      "Tip: Navigate deeper with -p \"0\", -p \"1\", etc. to locate target before editing.".dimmed()
    );
  } else {
    println!(
      "\n{}",
      format!("Tip: To modify this node, use `edit operate-at {namespace}/{definition} -p \"{path}\" -o replace '<cirru>'`").dimmed()
    );
    println!("{}", "     Use `-j '<json>'` for JSON input instead of Cirru.".dimmed());
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

/// Read examples of a definition
fn handle_read_examples(input_path: &str, namespace: &str, definition: &str) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found in namespace '{namespace}'"))?;

  println!("{} {}/{}", "Examples for:".bold(), namespace.cyan(), definition.green());

  if code_entry.examples.is_empty() {
    println!("\n{}", "(no examples)".dimmed());
    println!(
      "\n{}",
      format!("Tip: Use `edit set-examples {namespace}/{definition}` to add examples.").dimmed()
    );
  } else {
    println!("{} example(s)\n", code_entry.examples.len());

    for (i, example) in code_entry.examples.iter().enumerate() {
      println!("{}", format!("[{}]:", i + 1).bold());

      // Show Cirru format
      let cirru_str = cirru_parser::format(&[example.clone()], true.into()).unwrap_or_else(|_| "(failed)".to_string());
      for line in cirru_str.lines().filter(|l| !l.trim().is_empty()) {
        println!("  {line}");
      }

      // Show JSON format
      let json = cirru_to_json(example);
      println!("  {} {}", "JSON:".dimmed(), serde_json::to_string(&json).unwrap().dimmed());
      println!();
    }

    println!(
      "{}",
      format!("Tip: Use `edit set-examples {namespace}/{definition}` to modify examples.").dimmed()
    );
  }

  Ok(())
}

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

  // Always show doc (even if empty)
  if code_entry.doc.is_empty() {
    println!("{} -", "Doc:".bold());
  } else {
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

        // Show first expression in Cirru format (compact inline)
        if items.len() > 3 {
          let first_body = &items[3];
          let cirru_preview = cirru_parser::format(&[first_body.clone()], true.into()).unwrap_or_else(|_| "(failed)".to_string());
          // Get non-empty first line
          let first_line = cirru_preview.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
          if !first_line.is_empty() {
            let display = if first_line.len() > 60 {
              format!("{}...", &first_line[..60])
            } else {
              first_line.to_string()
            };
            println!("{} {}", "Body start:".bold(), display.dimmed());
          }
        }
      } else if form_type == "def" && items.len() >= 3 {
        // For def, show value preview in Cirru
        let value = &items[2];
        let cirru_preview = cirru_parser::format(&[value.clone()], true.into()).unwrap_or_else(|_| "(failed)".to_string());
        let first_line = cirru_preview.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
        if !first_line.is_empty() {
          let display = if first_line.len() > 60 {
            format!("{}...", &first_line[..60])
          } else {
            first_line.to_string()
          };
          println!("{} {}", "Value:".bold(), display.dimmed());
        }
      }
    }
    _ => {
      println!("{}", "(empty or invalid definition)".dimmed());
    }
  }

  // Always show examples count
  println!("{} {}", "Examples:".bold(), code_entry.examples.len());

  // LLM guidance - show relevant next steps
  println!("\n{}", "Tips:".bold());
  println!("  {} query read-def {}/{}", "-".dimmed(), namespace, definition);
  println!("  {} query read-examples {}/{}", "-".dimmed(), namespace, definition);
  println!("  {} query usages {}/{}", "-".dimmed(), namespace, definition);
  println!("  {} edit update-def-doc {}/{} '<doc>'", "-".dimmed(), namespace, definition);

  Ok(())
}

/// Find symbol across all namespaces
fn handle_find_symbol(input_path: &str, symbol: &str, include_deps: bool) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

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
        found_references.push((
          ns_name.clone(),
          def_name.clone(),
          get_symbol_context_cirru(&code_entry.code, symbol),
        ));
      }
    }
  }

  // Print summary
  println!(
    "{} '{}' - {} definition(s), {} reference(s)\n",
    "Symbol:".bold(),
    symbol.yellow(),
    found_definitions.len(),
    found_references.len().saturating_sub(found_definitions.len())
  );

  // Print definitions
  if !found_definitions.is_empty() {
    println!("{}", "Defined in:".bold().green());
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
      if !context.is_empty() {
        println!("  {}/{}  {}", ns.cyan(), def, context.dimmed());
      } else {
        println!("  {}/{}", ns.cyan(), def);
      }
    }
  }

  if found_definitions.is_empty() && references.is_empty() {
    println!("{}", "No matches found.".yellow());
    println!("\n{}", "Tip: Try `query ls-ns` to see available namespaces.".dimmed());
  } else if !found_definitions.is_empty() {
    // LLM guidance
    let (first_ns, first_def) = &found_definitions[0];
    println!(
      "\n{}",
      format!("Tip: Use `query peek-def {first_ns}/{first_def}` to see signature.").dimmed()
    );
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
        let context = get_symbol_context_cirru(&code_entry.code, target_def);
        usages.push((ns_name.clone(), def_name.clone(), context));
      }
    }
  }

  println!(
    "{} {}/{}  ({} usages)",
    "Usages of:".bold(),
    target_ns.cyan(),
    target_def.green(),
    usages.len()
  );

  if usages.is_empty() {
    println!(
      "\n{}",
      "No usages found. This definition may be unused or only called externally.".yellow()
    );
  } else {
    println!();
    for (ns, def, context) in &usages {
      if !context.is_empty() {
        println!("  {}/{}  {}", ns.cyan(), def.green(), context.dimmed());
      } else {
        println!("  {}/{}", ns.cyan(), def.green());
      }
    }
  }

  // LLM guidance
  if !usages.is_empty() {
    println!(
      "\n{}",
      "Tip: Modifying this definition may affect the above locations. Review before refactoring.".dimmed()
    );
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

// Helper: get context around symbol usage in Cirru format (compact)
// Returns the smallest expression containing the symbol
fn get_symbol_context_cirru(code: &Cirru, symbol: &str) -> String {
  fn find_smallest_containing(node: &Cirru, symbol: &str) -> Option<Cirru> {
    match node {
      Cirru::Leaf(s) if s.as_ref() == symbol => Some(node.clone()),
      Cirru::List(items) => {
        // First, try to find a smaller match in children (skip first which is usually form name)
        for item in items.iter().skip(1) {
          if let Some(found) = find_smallest_containing(item, symbol) {
            // If it's a direct leaf match, return parent expression for context
            if matches!(found, Cirru::Leaf(_)) {
              // Return current node as context
              return Some(node.clone());
            }
            return Some(found);
          }
        }
        // Check if symbol is in first position
        if let Some(Cirru::Leaf(s)) = items.first() {
          if s.as_ref() == symbol {
            return Some(node.clone());
          }
        }
        None
      }
      _ => None,
    }
  }

  if let Some(context_node) = find_smallest_containing(code, symbol) {
    let cirru_str = cirru_parser::format(&[context_node], true.into()).unwrap_or_default();
    // Get first non-empty line
    let first_line = cirru_str.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    if first_line.len() > 50 {
      return format!("{}...", &first_line[..50]);
    }
    return first_line.to_string();
  }
  String::new()
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

/// Fuzzy search for namespace/definition by pattern
/// Searches for `<pattern>` in qualified names like `namespace/definition`
fn handle_fuzzy_search(input_path: &str, pattern: &str, include_deps: bool, limit: usize) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let pattern_lower = pattern.to_lowercase();
  let mut results: Vec<(String, String, bool)> = Vec::new(); // (namespace, def, is_core)

  // Search in all files
  for (ns_name, file_data) in &snapshot.files {
    // Mark core namespaces as "dep" for display
    let is_core = ns_name.starts_with("calcit.") || ns_name.starts_with("calcit-test.");

    // Skip core namespaces unless deps is requested
    if !include_deps && is_core {
      continue;
    }

    for def_name in file_data.defs.keys() {
      let qualified = format!("{ns_name}/{def_name}");
      let qualified_lower = qualified.to_lowercase();

      // Fuzzy match: check if pattern appears in qualified name
      if fuzzy_match(&qualified_lower, &pattern_lower) {
        results.push((ns_name.clone(), def_name.clone(), is_core));
      }
    }
  }

  // Sort results by relevance (exact prefix match first, then alphabetically)
  results.sort_by(|(ns_a, def_a, _), (ns_b, def_b, _)| {
    let qualified_a = format!("{ns_a}/{def_a}").to_lowercase();
    let qualified_b = format!("{ns_b}/{def_b}").to_lowercase();

    // Prioritize exact prefix matches
    let a_prefix = qualified_a.starts_with(&pattern_lower);
    let b_prefix = qualified_b.starts_with(&pattern_lower);

    match (a_prefix, b_prefix) {
      (true, false) => std::cmp::Ordering::Less,
      (false, true) => std::cmp::Ordering::Greater,
      _ => qualified_a.cmp(&qualified_b),
    }
  });

  // Limit results
  let total = results.len();
  let displayed: Vec<_> = results.into_iter().take(limit).collect();

  println!("{} {} results for pattern \"{}\"", "Search:".bold(), total, pattern.yellow());

  if displayed.is_empty() {
    println!("  {}", "No matches found".dimmed());
    println!(
      "\n{}",
      "Tip: Try a broader pattern, or add --deps to include core namespaces.".dimmed()
    );
    return Ok(());
  }

  for (ns, def, is_core) in &displayed {
    let qualified = format!("{}/{}", ns.cyan(), def.green());
    if *is_core {
      println!("  {} {}", qualified, "(core)".dimmed());
    } else {
      println!("  {qualified}");
    }
  }

  if total > limit {
    println!("  {} {} more results...", "...".dimmed(), total - limit);
  }

  println!("\n{}", "Tip: Use `query read-def <ns/def>` to view definition content.".dimmed());

  Ok(())
}

/// Simple fuzzy matching: check if all characters of pattern appear in order in text
fn fuzzy_match(text: &str, pattern: &str) -> bool {
  // Support multiple match styles:
  // 1. Substring match: "map" matches "hash-map"
  // 2. Character sequence match: "hm" matches "hash-map"

  // First try substring match (fast path)
  if text.contains(pattern) {
    return true;
  }

  // Then try character sequence match
  let mut text_chars = text.chars().peekable();
  for pattern_char in pattern.chars() {
    loop {
      match text_chars.next() {
        Some(c) if c == pattern_char => break,
        Some(_) => continue,
        None => return false,
      }
    }
  }
  true
}
