//! Query subcommand handlers
//!
//! Handles: cr query ns, defs, def, at, peek, examples, find, usages, pkg, config, error, modules

use calcit::cli_args::{QueryCommand, QuerySubcommand};
use calcit::load_core_snapshot;
use calcit::snapshot;
use calcit::util::string::strip_shebang;
use cirru_parser::Cirru;
use colored::Colorize;
use std::fs;
use std::path::Path;

use super::edit::navigate_to_path;

/// Type alias for search results: (namespace, definition, matches)
type SearchResults = Vec<(String, String, Vec<(Vec<usize>, Cirru)>)>;

/// Parse "namespace/definition" format into (namespace, definition)
fn parse_target(target: &str) -> Result<(&str, &str), String> {
  target
    .rsplit_once('/')
    .ok_or_else(|| format!("Invalid target format: '{target}'. Expected 'namespace/definition' (e.g. 'app.core/main')"))
}

pub fn handle_query_command(cmd: &QueryCommand, input_path: &str) -> Result<(), String> {
  match &cmd.subcommand {
    QuerySubcommand::Ns(opts) => handle_ns(input_path, opts.namespace.as_deref(), opts.deps),
    QuerySubcommand::Defs(opts) => handle_defs(input_path, &opts.namespace),
    QuerySubcommand::Pkg(_) => handle_pkg(input_path),
    QuerySubcommand::Config(_) => handle_config(input_path),
    QuerySubcommand::Error(_) => handle_error(),
    QuerySubcommand::Modules(_) => handle_modules(input_path),
    QuerySubcommand::Def(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_def(input_path, ns, def, opts.json)
    }
    QuerySubcommand::Peek(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_peek(input_path, ns, def)
    }
    QuerySubcommand::Examples(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_examples(input_path, ns, def)
    }
    QuerySubcommand::Find(opts) => {
      if opts.fuzzy {
        handle_fuzzy_search(input_path, &opts.symbol, opts.deps, opts.limit)
      } else {
        handle_find(input_path, &opts.symbol, opts.deps)
      }
    }
    QuerySubcommand::Usages(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_usages(input_path, ns, def, opts.deps)
    }
    QuerySubcommand::Search(opts) => handle_search_leaf(
      input_path,
      &opts.pattern,
      opts.filter.as_deref(),
      opts.loose,
      opts.max_depth,
      opts.start_path.as_deref(),
    ),
    QuerySubcommand::SearchExpr(opts) => handle_search_expr(
      input_path,
      &opts.pattern,
      opts.filter.as_deref(),
      opts.loose,
      opts.max_depth,
      opts.json,
    ),
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
  let data = cirru_edn::parse(&content).map_err(|e| {
    eprintln!("\nFailed to parse file '{}':", fullpath.display());
    eprintln!("{e}");
    format!("Failed to parse file '{}'", fullpath.display())
  })?;
  snapshot::load_snapshot_data(&data, &fullpath.display().to_string())
}

fn load_snapshot(input_path: &str) -> Result<snapshot::Snapshot, String> {
  if !Path::new(input_path).exists() {
    return Err(format!("{input_path} does not exist"));
  }

  let mut content = fs::read_to_string(input_path).map_err(|e| format!("Failed to read file: {e}"))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content).map_err(|e| {
    eprintln!("\nFailed to parse file '{input_path}':");
    eprintln!("{e}");
    format!("Failed to parse file '{input_path}'")
  })?;
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

/// Handle `query ns` - list namespaces or show ns details
fn handle_ns(input_path: &str, namespace: Option<&str>, include_deps: bool) -> Result<(), String> {
  // If namespace is provided, show details (merged read-ns functionality)
  if let Some(ns_name) = namespace {
    return handle_ns_details(input_path, ns_name);
  }

  // Otherwise list all namespaces
  if !Path::new(input_path).exists() {
    return Err(format!("{input_path} does not exist"));
  }

  let mut content = fs::read_to_string(input_path).map_err(|e| format!("Failed to read file: {e}"))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content).map_err(|e| {
    eprintln!("\nFailed to parse file '{input_path}':");
    eprintln!("{e}");
    format!("Failed to parse file '{input_path}'")
  })?;
  let main_snapshot = snapshot::load_snapshot_data(&data, input_path)?;
  let main_package = main_snapshot.package.clone();

  let snapshot = if include_deps { load_snapshot(input_path)? } else { main_snapshot };

  let mut namespaces: Vec<&String> = snapshot.files.keys().collect();
  namespaces.sort();

  let filtered: Vec<_> = namespaces
    .iter()
    .filter(|ns| {
      if !include_deps {
        ns.as_str() == main_package || ns.starts_with(&format!("{main_package}."))
      } else {
        true
      }
    })
    .collect();

  println!(
    "{} ({} namespaces)",
    if include_deps { "All namespaces:" } else { "Project namespaces:" }.bold(),
    filtered.len()
  );

  for ns in &filtered {
    println!("  {}", ns.cyan());
  }

  if !include_deps {
    println!("\n{}", "Tip: Use `--deps` to include dependency and core namespaces.".dimmed());
  }
  println!("{}", "Tip: Use `cr query ns <namespace>` to show namespace details.".dimmed());
  println!("{}", "Tip: Use `cr query defs <namespace>` to list definitions.".dimmed());

  Ok(())
}

/// Handle `query ns <namespace>` - show ns details
fn handle_ns_details(input_path: &str, namespace: &str) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  println!("{} {}", "Namespace:".bold(), namespace.cyan());

  if !file_data.ns.doc.is_empty() {
    println!("{} {}", "Doc:".bold(), file_data.ns.doc);
  }

  println!("\n{}", "NS declaration:".bold());
  let ns_str = cirru_parser::format(&[file_data.ns.code.clone()], true.into()).unwrap_or_else(|_| "(failed to format)".to_string());
  println!("{}", ns_str.dimmed());

  println!("\n{} {}", "Definitions:".bold(), file_data.defs.len());

  println!(
    "\n{}",
    format!("Tip: Use `cr query defs {namespace}` to list definitions.").dimmed()
  );

  Ok(())
}

fn handle_defs(input_path: &str, namespace: &str) -> Result<(), String> {
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
      let doc_first_line = entry.doc.lines().next().unwrap_or("");
      let doc_display = if doc_first_line.len() > 50 {
        format!("{}...", &doc_first_line[..50])
      } else {
        doc_first_line.to_string()
      };
      println!("  {} - {}", def.green(), doc_display.dimmed());
    } else {
      println!("  {}", def.green());
    }
  }

  println!(
    "\n{}",
    "Tip: Use `cr query peek <ns/def>` for signature, `cr query def <ns/def>` for full code.".dimmed()
  );

  Ok(())
}

fn handle_pkg(input_path: &str) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;
  println!("{}", snapshot.package);
  Ok(())
}

fn handle_config(input_path: &str) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  println!("{}", "Project Configs:".bold());
  println!("  {}: {}", "init_fn".cyan(), snapshot.configs.init_fn);
  println!("  {}: {}", "reload_fn".cyan(), snapshot.configs.reload_fn);
  println!("  {}: {}", "version".cyan(), snapshot.configs.version);
  println!("  {}: {:?}", "modules".cyan(), snapshot.configs.modules);

  if !snapshot.entries.is_empty() {
    println!("\n{}", "Snapshot Entries:".bold());

    let mut names: Vec<&String> = snapshot.entries.keys().collect();
    names.sort();

    for name in names {
      let entry = snapshot
        .entries
        .get(name)
        .ok_or_else(|| format!("Missing entry config for '{name}'"))?;

      println!("  {}", name.cyan());
      println!("    {}: {}", "init_fn".cyan(), entry.init_fn);
      println!("    {}: {}", "reload_fn".cyan(), entry.reload_fn);
      println!("    {}: {}", "version".cyan(), entry.version);
      println!("    {}: {:?}", "modules".cyan(), entry.modules);
    }
  }

  Ok(())
}

fn handle_error() -> Result<(), String> {
  let error_file = ".calcit-error.cirru";

  if !Path::new(error_file).exists() {
    println!("{}", "No .calcit-error.cirru file found.".yellow());
    println!();
    println!("{}", "Next steps:".blue().bold());
    println!("  • Start watcher: {} or {}", "cr".cyan(), "cr js".cyan());
    println!("  • Run syntax check: {}", "cr --check-only".cyan());
    return Ok(());
  }

  let content = fs::read_to_string(error_file).map_err(|e| format!("Failed to read error file: {e}"))?;

  if content.trim().is_empty() {
    println!("{}", "✓ Error file is empty (no recent errors).".green());
    println!();
    println!("{}", "Your code compiled successfully!".dimmed());
  } else {
    println!("{}", "Last error stack trace:".bold().red());
    println!("{content}");
    println!();
    println!("{}", "Next steps to fix:".blue().bold());
    println!("  • Search for error location: {} '<symbol>' -l", "cr query search".cyan());
    println!("  • View definition: {} '<ns/def>'", "cr query def".cyan());
    println!("  • Find usages: {} '<ns/def>'", "cr query usages".cyan());
    println!();
    println!("{}", "Tip: After fixing, watcher will recompile automatically (~300ms).".dimmed());
  }

  Ok(())
}

fn handle_modules(input_path: &str) -> Result<(), String> {
  if !Path::new(input_path).exists() {
    return Err(format!("{input_path} does not exist"));
  }

  let mut content = fs::read_to_string(input_path).map_err(|e| format!("Failed to read file: {e}"))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content).map_err(|e| {
    eprintln!("\nFailed to parse file '{input_path}':");
    eprintln!("{e}");
    format!("Failed to parse file '{input_path}'")
  })?;
  let snapshot = snapshot::load_snapshot_data(&data, input_path)?;

  let base_dir = Path::new(input_path).parent().unwrap_or(Path::new("."));
  let module_folder = dirs::home_dir()
    .map(|buf| buf.as_path().join(".config/calcit/modules/"))
    .unwrap_or_else(|| Path::new(".").to_owned());

  println!("{}", "Modules in project:".bold());

  println!("  {} {}", snapshot.package.cyan(), "(main)".dimmed());

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

  if !snapshot.entries.is_empty() {
    println!("\n{}", "Entries:".bold());
    for name in snapshot.entries.keys() {
      println!("  {}", name.cyan());
    }
  }

  println!(
    "\n{}",
    "Tip: Use `cr query ns` to list namespaces, `cr query defs <namespace>` to list definitions.".dimmed()
  );

  Ok(())
}

fn handle_def(input_path: &str, namespace: &str, definition: &str, show_json: bool) -> Result<(), String> {
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

  if !code_entry.doc.is_empty() {
    println!("{} {}", "Doc:".bold(), code_entry.doc);
  }

  if !code_entry.examples.is_empty() {
    println!("\n{} {}", "Examples:".bold(), code_entry.examples.len());
  }

  println!("\n{}", "Cirru:".bold());
  let cirru_str = cirru_parser::format(&[code_entry.code.clone()], true.into()).unwrap_or_else(|_| "(failed to format)".to_string());
  println!("{cirru_str}");

  if show_json {
    println!("\n{}", "JSON:".bold());
    let json = cirru_to_json(&code_entry.code);
    println!("{}", serde_json::to_string(&json).unwrap());
  }

  let mut tips = vec![format!(
    "try `cr query search <leaf> -f '{namespace}/{definition}' -l` to quick find coordination of given leaf node."
  )];
  tips.push(format!(
    "use `cr tree show {namespace}/{definition} -p '0'` to explore tree for editing."
  ));
  if !code_entry.examples.is_empty() {
    tips.push(format!("use `cr query examples {namespace}/{definition}` to view examples."));
  }
  if !show_json {
    tips.push("add `-j` flag to also output JSON format.".to_string());
  }
  println!("\n{}", format!("Tips: {}", tips.join(" ")).dimmed());

  Ok(())
}

fn cirru_to_json(cirru: &Cirru) -> serde_json::Value {
  match cirru {
    Cirru::Leaf(s) => serde_json::Value::String(s.to_string()),
    Cirru::List(items) => serde_json::Value::Array(items.iter().map(cirru_to_json).collect()),
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Progressive disclosure commands
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_examples(input_path: &str, namespace: &str, definition: &str) -> Result<(), String> {
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
      format!("Tip: Use `cr edit examples {namespace}/{definition}` to add examples.").dimmed()
    );
  } else {
    println!("{} example(s)\n", code_entry.examples.len());

    for (i, example) in code_entry.examples.iter().enumerate() {
      println!("{}", format!("[{i}]:").bold());

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
      format!("Tip: Use `cr edit examples {namespace}/{definition}` to modify examples.").dimmed()
    );
  }

  Ok(())
}

/// Peek definition - show signature/params/doc without full body
fn handle_peek(input_path: &str, namespace: &str, definition: &str) -> Result<(), String> {
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
      // Show entire definition as one-liner, truncated to 120 chars
      let preview = code_entry.code.format_one_liner()?;
      let display = if preview.len() > 120 {
        format!("{}...", &preview[..120])
      } else {
        preview
      };
      println!("{} {}", "Expr:".bold(), display.dimmed());
    }
    Cirru::Leaf(_) => {
      // Single leaf definition
      let preview = code_entry.code.format_one_liner()?;
      println!("{} {}", "Leaf:".bold(), preview.dimmed());
    }
    _ => {
      println!("{}", "(empty or invalid definition)".dimmed());
    }
  }

  // Always show examples count
  println!("{} {}", "Examples:".bold(), code_entry.examples.len());

  // Tips - show relevant next steps
  println!("\n{}", "Tips:".bold());
  println!("  {} cr query def {}/{}", "-".dimmed(), namespace, definition);
  println!("  {} cr query examples {}/{}", "-".dimmed(), namespace, definition);
  println!("  {} cr query usages {}/{}", "-".dimmed(), namespace, definition);
  println!("  {} cr edit doc {}/{} '<doc>'", "-".dimmed(), namespace, definition);
  println!(
    "  {} Respo: event handlers go inside {} map; strings need {} prefix",
    "-".dimmed(),
    "attributes".green(),
    "|".magenta()
  );

  Ok(())
}

/// Find symbol across all namespaces
fn handle_find(input_path: &str, symbol: &str, include_deps: bool) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let mut found_definitions: Vec<(String, String)> = vec![];
  let mut found_references: Vec<(String, String, String, Vec<Vec<usize>>)> = vec![]; // (ns, def, context, coords)

  for (ns_name, file_data) in &snapshot.files {
    let is_core = ns_name.starts_with("calcit.") || ns_name.starts_with("calcit-test.");

    // Always search for definitions in all namespaces (including core)
    if file_data.defs.contains_key(symbol) {
      found_definitions.push((ns_name.clone(), symbol.to_string()));
    }

    // Search for references only in project namespaces (unless --deps)
    if !include_deps && is_core {
      continue;
    }

    // Search for references in all definitions
    for (def_name, code_entry) in &file_data.defs {
      if find_symbol_in_cirru(&code_entry.code, symbol) {
        let coords = find_symbol_coords(&code_entry.code, symbol);
        found_references.push((
          ns_name.clone(),
          def_name.clone(),
          get_symbol_context_cirru(&code_entry.code, symbol),
          coords,
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
    .filter(|(ns, def, _, _)| !found_definitions.iter().any(|(dns, ddef)| dns == ns && ddef == def))
    .collect();

  if !references.is_empty() {
    println!("{}", "Referenced in:".bold());
    for (ns, def, context, coords) in &references {
      // Show main line
      if !context.is_empty() {
        println!("  {}/{}  {}", ns.cyan(), def, context.dimmed());
      } else {
        println!("  {}/{}", ns.cyan(), def);
      }

      // Show coordinates on one line with "and" separator
      if !coords.is_empty() {
        let coords_parts: Vec<String> = coords
          .iter()
          .map(|path| {
            let coord_str = path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
            format!("[{coord_str}]")
          })
          .collect();
        println!("    {}", format!("at {}", coords_parts.join(" and ")).dimmed());
      }
    }
  }

  if found_definitions.is_empty() && references.is_empty() {
    println!("{}", "No matches found.".yellow());
    println!("\n{}", "Tip: Try `cr query ns` to see available namespaces.".dimmed());
  } else if !found_definitions.is_empty() {
    let (first_ns, first_def) = &found_definitions[0];
    println!(
      "\n{}",
      format!("Tip: Use `cr query peek {first_ns}/{first_def}` to see signature.").dimmed()
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

  let mut usages: Vec<(String, String, String, Vec<Vec<usize>>)> = vec![]; // (ns, def, context, coords)

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
        find_symbol_in_cirru(&code_entry.code, target_def)
      } else {
        // Check for qualified reference: target_ns/target_def
        let qualified = format!("{target_ns}/{target_def}");
        find_symbol_in_cirru(&code_entry.code, &qualified)
      };

      if found {
        let context = get_symbol_context_cirru(&code_entry.code, target_def);
        // Get all coordinates where the symbol appears
        let coords = if imports_target || ns_name == target_ns {
          find_symbol_coords(&code_entry.code, target_def)
        } else {
          let qualified = format!("{target_ns}/{target_def}");
          find_symbol_coords(&code_entry.code, &qualified)
        };
        usages.push((ns_name.clone(), def_name.clone(), context, coords));
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
    for (ns, def, context, coords) in &usages {
      // Show main line
      if !context.is_empty() {
        println!("  {}/{}  {}", ns.cyan(), def.green(), context.dimmed());
      } else {
        println!("  {}/{}", ns.cyan(), def.green());
      }

      // Show coordinates on one line with "and" separator
      if !coords.is_empty() {
        let coords_parts: Vec<String> = coords
          .iter()
          .map(|path| {
            let coord_str = path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
            format!("[{coord_str}]")
          })
          .collect();
        println!("    {}", format!("at {}", coords_parts.join(" and ")).dimmed());
      }
    }
  }

  // Tip
  if !usages.is_empty() {
    println!("\n{}", "Tip: Modifying this definition may affect the above locations.".dimmed());
  }

  Ok(())
}

// Helper: find all coordinates where symbol appears in Cirru tree
fn find_symbol_coords(code: &Cirru, symbol: &str) -> Vec<Vec<usize>> {
  fn search_recursive(node: &Cirru, symbol: &str, current_path: &[usize], results: &mut Vec<Vec<usize>>) {
    match node {
      Cirru::Leaf(s) if s.as_ref() == symbol => {
        results.push(current_path.to_vec());
      }
      Cirru::List(items) => {
        for (i, item) in items.iter().enumerate() {
          let mut new_path = current_path.to_vec();
          new_path.push(i);
          search_recursive(item, symbol, &new_path, results);
        }
      }
      _ => {}
    }
  }

  let mut results = Vec::new();
  search_recursive(code, symbol, &[], &mut results);
  results
}

// Helper: recursively search for symbol in Cirru tree
fn find_symbol_in_cirru(code: &Cirru, symbol: &str) -> bool {
  match code {
    Cirru::Leaf(s) => s.as_ref() == symbol,
    Cirru::List(items) => items.iter().any(|item| find_symbol_in_cirru(item, symbol)),
  }
}

// Helper: get context around symbol usage in Cirru format (compact)
// Returns the smallest expression containing the symbol
fn get_symbol_context_cirru(code: &Cirru, symbol: &str) -> String {
  fn find_smallest_containing(node: &Cirru, symbol: &str) -> Option<Cirru> {
    match node {
      Cirru::Leaf(s) if s.as_ref() == symbol => Some(node.clone()),
      Cirru::List(items) => {
        for item in items {
          if let Some(found) = find_smallest_containing(item, symbol) {
            if matches!(found, Cirru::Leaf(_)) {
              return Some(node.clone());
            }
            return Some(found);
          }
        }
        None
      }
      _ => None,
    }
  }

  if let Some(context_node) = find_smallest_containing(code, symbol) {
    let cirru_str = context_node.format_one_liner().unwrap_or_default();
    let trimmed = cirru_str.trim();
    if trimmed.len() > 50 {
      return format!("{}...", &trimmed[..50]);
    }
    return trimmed.to_string();
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

  println!("\n{}", "Tip: Use `query def <ns/def>` to view definition content.".dimmed());

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

/// Search for leaf nodes (strings) in a definition
fn handle_search_leaf(
  input_path: &str,
  pattern: &str,
  filter: Option<&str>,
  loose: bool,
  max_depth: usize,
  start_path: Option<&str>,
) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  // Parse start_path if provided
  let parsed_start_path: Option<Vec<usize>> = if let Some(path_str) = start_path {
    if path_str.is_empty() {
      Some(vec![])
    } else {
      let path: Result<Vec<usize>, _> = path_str.split(',').map(|s| s.trim().parse::<usize>()).collect();
      Some(path.map_err(|e| format!("Invalid start path '{path_str}': {e}"))?)
    }
  } else {
    None
  };

  println!("{} Searching for:", "Search:".bold());
  if loose {
    println!("  {} (contains)", pattern.yellow());
  } else {
    println!("  {} (exact)", pattern.yellow());
  }

  if let Some(filter_str) = filter {
    println!("  {} {}", "Filter:".dimmed(), filter_str.cyan());
  } else {
    println!("  {} {}", "Scope:".dimmed(), "entire project".cyan());
  }

  if let Some(ref path) = parsed_start_path {
    let path_display = if path.is_empty() {
      "root".to_string()
    } else {
      format!("[{}]", path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","))
    };
    println!("  {} {}", "Start path:".dimmed(), path_display.cyan());
  }
  println!();

  let mut all_results: SearchResults = Vec::new();

  // Parse filter to determine scope
  let (filter_ns, filter_def) = if let Some(f) = filter {
    if f.contains('/') {
      let parts: Vec<&str> = f.split('/').collect();
      if parts.len() == 2 {
        (Some(parts[0]), Some(parts[1]))
      } else {
        return Err(format!("Invalid filter format: '{f}'. Use 'namespace' or 'namespace/definition'"));
      }
    } else {
      (Some(f), None)
    }
  } else {
    (None, None)
  };

  // Search through files
  for (ns, file_data) in &snapshot.files {
    // Skip if namespace doesn't match filter
    if let Some(filter_namespace) = filter_ns {
      if ns != filter_namespace {
        continue;
      }
    }

    // Search through definitions in this namespace
    for (def_name, code_entry) in &file_data.defs {
      // Skip if definition doesn't match filter
      if let Some(filter_definition) = filter_def {
        if def_name != filter_definition {
          continue;
        }
      }

      // Navigate to start path if specified
      let search_root = if let Some(ref start_p) = parsed_start_path {
        if start_p.is_empty() {
          code_entry.code.clone()
        } else {
          match navigate_to_path(&code_entry.code, start_p) {
            Ok(node) => node,
            Err(e) => {
              eprintln!(
                "{} Failed to navigate to start path in {}/{}: {}",
                "Warning:".yellow(),
                ns,
                def_name,
                e
              );
              continue;
            }
          }
        }
      } else {
        code_entry.code.clone()
      };

      let base_path = parsed_start_path.as_deref().unwrap_or(&[]);
      let results = search_leaf_nodes(&search_root, pattern, loose, max_depth, base_path);

      if !results.is_empty() {
        all_results.push((ns.clone(), def_name.clone(), results));
      }
    }
  }

  // Print results grouped by namespace/definition
  if all_results.is_empty() {
    println!("{}", "No matches found.".yellow());
  } else {
    let total_matches: usize = all_results.iter().map(|(_, _, results)| results.len()).sum();
    println!(
      "{} {} match(es) found in {} definition(s):\n",
      "Results:".bold().green(),
      total_matches,
      all_results.len()
    );

    for (ns, def_name, results) in &all_results {
      println!("{} {}/{} ({} matches)", "●".cyan(), ns.dimmed(), def_name.green(), results.len());

      // Load code_entry to print results
      if let Some(file_data) = snapshot.files.get(ns) {
        if let Some(code_entry) = file_data.defs.get(def_name) {
          for (path, _node) in results.iter().take(5) {
            if path.is_empty() {
              let content = code_entry.code.format_one_liner().unwrap_or_default();
              println!("    {} {}", "(root)".cyan(), content.dimmed());
            } else {
              let path_str = format!("[{}]", path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","));
              let breadcrumb = get_breadcrumb_from_code(&code_entry.code, path);
              println!("    {} {}", path_str.cyan(), breadcrumb.dimmed());

              // Get parent context
              if let Some(parent) = get_parent_node_from_code(&code_entry.code, path) {
                let parent_oneliner = parent.format_one_liner().unwrap_or_default();
                let display_parent = if parent_oneliner.len() > 80 {
                  format!("{}...", &parent_oneliner[..80])
                } else {
                  parent_oneliner
                };
                println!("      {} {}", "in".dimmed(), display_parent.dimmed());
              }
            }
          }

          if results.len() > 5 {
            println!("    {}", format!("... and {} more", results.len() - 5).dimmed());
          }
        }
      }
      println!();
    }

    // Enhanced tips based on search context
    println!("{}", "Next steps:".blue().bold());
    if all_results.len() == 1 && all_results[0].2.len() == 1 {
      let (ns, def_name, results) = &all_results[0];
      let (path, _) = &results[0];
      let path_str = path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
      println!("  • View node: {} '{}/{}' -p '{}'", "cr tree show".cyan(), ns, def_name, path_str);
      println!(
        "  • Replace: {} '{}/{}' -p '{}' --leaf -e '<new-value>'",
        "cr tree replace".cyan(),
        ns,
        def_name,
        path_str
      );
    } else {
      println!("  • View node: {} '<ns/def>' -p '<path>'", "cr tree show".cyan());
    }

    // If single definition with multiple matches, suggest batch rename workflow
    if all_results.len() == 1 {
      let (_ns, _def_name, results) = &all_results[0];
      if results.len() > 1 {
        println!("  • Batch replace: See tip below for renaming {} occurrences", results.len());
      }
    }

    println!();

    // Add batch rename tip for multiple matches in single definition
    if all_results.len() == 1 && all_results[0].2.len() > 1 {
      let (ns, def_name, results) = &all_results[0];
      println!("{}", "Tip for batch rename:".yellow().bold());
      println!("  Replace from largest index first to avoid path changes:");

      // Show first 3 commands as examples (in reverse order)
      let mut sorted_results: Vec<_> = results.iter().collect();
      sorted_results.sort_by(|a, b| b.0.cmp(&a.0));

      for (path, _) in sorted_results.iter().take(3) {
        let path_str = path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        println!(
          "    {} '{}/{}' -p '{}' --leaf -e '<new-value>'",
          "cr tree replace".cyan(),
          ns,
          def_name,
          path_str
        );
      }

      if results.len() > 3 {
        println!("    {}", format!("... ({} more to replace)", results.len() - 3).dimmed());
      }

      println!();
      println!("{}", "⚠️  Important: Paths change after each modification!".yellow());
      println!(
        "{}",
        format!(
          "   Alternative: Re-search after each change: {} '{}' -f '{}/{}' -l",
          "cr query search".cyan(),
          pattern,
          ns,
          def_name
        )
        .dimmed()
      );
    }

    // Add quote reminder if pattern contains special characters
    if pattern.contains('-') || pattern.contains('?') || pattern.contains('!') || pattern.contains('*') {
      println!();
      println!(
        "{}",
        format!("Tip: Always use single quotes around names with special characters: '{pattern}'").dimmed()
      );
    }
  }

  Ok(())
}

/// Search for structural expressions across project or in filtered scope
fn handle_search_expr(
  input_path: &str,
  pattern: &str,
  filter: Option<&str>,
  loose: bool,
  max_depth: usize,
  json: bool,
) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  // Parse pattern
  let pattern_node = if json {
    // Parse as JSON array
    let json_val: serde_json::Value = serde_json::from_str(pattern).map_err(|e| format!("Failed to parse JSON pattern: {e}"))?;
    json_to_cirru(&json_val)?
  } else {
    // Parse as Cirru one-liner
    cirru_parser::parse(pattern)
      .map_err(|e| format!("Failed to parse Cirru pattern: {e}"))?
      .first()
      .ok_or("Pattern is empty")?
      .clone()
  };

  println!("{} Searching for pattern:", "Search:".bold());

  let pattern_display = pattern_node.format_one_liner().unwrap_or_default();
  if loose {
    println!("  {} (contains sequence)", pattern_display.yellow());
  } else {
    println!("  {} (exact match)", pattern_display.yellow());
  }

  if let Some(filter_str) = filter {
    println!("  {} {}", "Filter:".dimmed(), filter_str.cyan());
  } else {
    println!("  {} {}", "Scope:".dimmed(), "entire project".cyan());
  }
  println!();

  let mut all_results: SearchResults = Vec::new();

  // Parse filter to determine scope
  let (filter_ns, filter_def) = if let Some(f) = filter {
    if f.contains('/') {
      let parts: Vec<&str> = f.split('/').collect();
      if parts.len() == 2 {
        (Some(parts[0]), Some(parts[1]))
      } else {
        return Err(format!("Invalid filter format: '{f}'. Use 'namespace' or 'namespace/definition'"));
      }
    } else {
      (Some(f), None)
    }
  } else {
    (None, None)
  };

  // Search through files
  for (ns, file_data) in &snapshot.files {
    // Skip if namespace doesn't match filter
    if let Some(filter_namespace) = filter_ns {
      if ns != filter_namespace {
        continue;
      }
    }

    // Search through definitions in this namespace
    for (def_name, code_entry) in &file_data.defs {
      // Skip if definition doesn't match filter
      if let Some(filter_definition) = filter_def {
        if def_name != filter_definition {
          continue;
        }
      }

      let results = search_expr_nodes(&code_entry.code, &pattern_node, loose, max_depth, &[]);

      if !results.is_empty() {
        all_results.push((ns.clone(), def_name.clone(), results));
      }
    }
  }

  // Print results grouped by namespace/definition
  if all_results.is_empty() {
    println!("{}", "No matches found.".yellow());
  } else {
    let total_matches: usize = all_results.iter().map(|(_, _, results)| results.len()).sum();
    println!(
      "{} {} match(es) found in {} definition(s):\n",
      "Results:".bold().green(),
      total_matches,
      all_results.len()
    );

    for (ns, def_name, results) in &all_results {
      println!("{} {}/{} ({} matches)", "●".cyan(), ns.dimmed(), def_name.green(), results.len());

      // Load code_entry to print results
      if let Some(file_data) = snapshot.files.get(ns) {
        if let Some(code_entry) = file_data.defs.get(def_name) {
          for (path, _node) in results.iter().take(5) {
            let path_str = path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
            if path.is_empty() {
              let content = code_entry.code.format_one_liner().unwrap_or_default();
              println!("    {} {}", "(root)".cyan(), content.dimmed());
            } else {
              let breadcrumb = get_breadcrumb_from_code(&code_entry.code, path);
              println!("    {} path: '{}' context: {}", "•".cyan(), path_str, breadcrumb.dimmed());

              // Get parent context
              if let Some(parent) = get_parent_node_from_code(&code_entry.code, path) {
                let parent_oneliner = parent.format_one_liner().unwrap_or_default();
                let display_parent = if parent_oneliner.len() > 80 {
                  format!("{}...", &parent_oneliner[..80])
                } else {
                  parent_oneliner
                };
                println!("      {} {}", "in:".dimmed(), display_parent.dimmed());
              }
            }
          }

          if results.len() > 5 {
            println!("    {}", format!("... and {} more", results.len() - 5).dimmed());
          }
        }
      }
      println!();
    }

    // Enhanced tips based on search context
    println!("{}", "Next steps:".blue().bold());
    if all_results.len() == 1 && all_results[0].2.len() == 1 {
      let (ns, def_name, results) = &all_results[0];
      let (path, _) = &results[0];
      let path_str = path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
      println!("  • View node: {} '{}/{}' -p '{}'", "cr tree show".cyan(), ns, def_name, path_str);
      println!(
        "  • Replace: {} '{}/{}' -p '{}' -e '<new-expression>'",
        "cr tree replace".cyan(),
        ns,
        def_name,
        path_str
      );
    } else {
      println!("  • View node: {} '<ns/def>' -p '<path>'", "cr tree show".cyan());
    }

    // If single definition with multiple matches, suggest batch replace workflow
    if all_results.len() == 1 {
      let (_ns, _def_name, results) = &all_results[0];
      if results.len() > 1 {
        println!("  • Batch replace: See tip below for renaming {} occurrences", results.len());
      }
    }

    println!();

    // Add batch replace tip for multiple matches in single definition
    if all_results.len() == 1 && all_results[0].2.len() > 1 {
      let (ns, def_name, results) = &all_results[0];
      println!("{}", "Tip for batch replace:".yellow().bold());
      println!("  Replace from largest index first to avoid path changes:");

      // Show first 3 commands as examples (in reverse order)
      let mut sorted_results: Vec<_> = results.iter().collect();
      sorted_results.sort_by(|a, b| b.0.cmp(&a.0));

      for (path, _) in sorted_results.iter().take(3) {
        let path_str = path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        println!(
          "    {} '{}/{}' -p '{}' -e '<new-expression>'",
          "cr tree replace".cyan(),
          ns,
          def_name,
          path_str
        );
      }

      if results.len() > 3 {
        println!("    {}", format!("... ({} more to replace)", results.len() - 3).dimmed());
      }
    }
  }

  Ok(())
}

/// Helper function to convert JSON to Cirru
fn json_to_cirru(json: &serde_json::Value) -> Result<Cirru, String> {
  match json {
    serde_json::Value::String(s) => Ok(Cirru::Leaf(s.as_str().into())),
    serde_json::Value::Array(arr) => {
      let items: Result<Vec<_>, _> = arr.iter().map(json_to_cirru).collect();
      Ok(Cirru::List(items?))
    }
    _ => Err("Pattern must be a string or array".to_string()),
  }
}

/// Print search results with parent context
/// Search for leaf nodes with exact or loose matching
fn search_leaf_nodes(node: &Cirru, pattern: &str, loose: bool, max_depth: usize, current_path: &[usize]) -> Vec<(Vec<usize>, Cirru)> {
  let mut results = Vec::new();

  // Check depth limit
  if max_depth > 0 && current_path.len() >= max_depth {
    return results;
  }

  // Only match leaf nodes
  match node {
    Cirru::Leaf(s) => {
      let matches = if loose {
        // Loose: check if leaf contains pattern
        s.to_lowercase().contains(&pattern.to_lowercase())
      } else {
        // Exact: check if leaf equals pattern
        s.as_ref() == pattern
      };

      if matches {
        results.push((current_path.to_vec(), node.clone()));
      }
    }
    Cirru::List(items) => {
      // Recursively search children
      for (i, item) in items.iter().enumerate() {
        let mut new_path = current_path.to_vec();
        new_path.push(i);
        results.extend(search_leaf_nodes(item, pattern, loose, max_depth, &new_path));
      }
    }
  }

  results
}

/// Helper function to get parent node from code given a path
fn get_parent_node_from_code(code: &Cirru, path: &[usize]) -> Option<Cirru> {
  if path.is_empty() {
    return None;
  }
  let parent_path = &path[..path.len() - 1];
  if parent_path.is_empty() {
    return Some(code.clone());
  }

  let mut current = code;
  for &idx in parent_path {
    if let Cirru::List(items) = current {
      current = items.get(idx)?;
    } else {
      return None;
    }
  }
  Some(current.clone())
}

fn get_breadcrumb_from_code(code: &Cirru, path: &[usize]) -> String {
  let mut parts = Vec::new();
  let mut current = code;

  parts.push(preview_cirru_head(current));

  for &idx in path {
    if let Cirru::List(items) = current {
      if let Some(next) = items.get(idx) {
        current = next;
        parts.push(preview_cirru_head(current));
      } else {
        break;
      }
    } else {
      break;
    }
  }

  parts.join(" → ")
}

fn preview_cirru_head(node: &Cirru) -> String {
  match node {
    Cirru::Leaf(s) => s.to_string(),
    Cirru::List(items) => {
      if items.is_empty() {
        "()".to_string()
      } else {
        match &items[0] {
          Cirru::Leaf(s) => s.to_string(),
          Cirru::List(_) => "(...)".to_string(),
        }
      }
    }
  }
}

/// Search for expression nodes (structural matching)
fn search_expr_nodes(node: &Cirru, pattern: &Cirru, loose: bool, max_depth: usize, current_path: &[usize]) -> Vec<(Vec<usize>, Cirru)> {
  let mut results = Vec::new();

  // Check depth limit
  if max_depth > 0 && current_path.len() >= max_depth {
    return results;
  }

  // Check if current node matches pattern
  let matches = if loose {
    contains_pattern(node, pattern)
  } else {
    matches_exact_structure(node, pattern)
  };

  if matches {
    results.push((current_path.to_vec(), node.clone()));
  }

  // Recursively search children
  if let Cirru::List(items) = node {
    for (i, item) in items.iter().enumerate() {
      let mut new_path = current_path.to_vec();
      new_path.push(i);
      results.extend(search_expr_nodes(item, pattern, loose, max_depth, &new_path));
    }
  }

  results
}

/// Check if node contains pattern as a contiguous subsequence
fn contains_pattern(node: &Cirru, pattern: &Cirru) -> bool {
  match (node, pattern) {
    // Leaf nodes: check string containment
    (Cirru::Leaf(s), Cirru::Leaf(p)) => s.to_lowercase().contains(&p.as_ref().to_lowercase()),

    // List containing pattern list as subsequence
    (Cirru::List(items), Cirru::List(pattern_items)) => {
      if pattern_items.is_empty() {
        return true;
      }

      // Check if pattern_items appears as a contiguous subsequence in items
      if pattern_items.len() > items.len() {
        return false;
      }

      for start_idx in 0..=(items.len() - pattern_items.len()) {
        let mut all_match = true;
        for (i, pattern_item) in pattern_items.iter().enumerate() {
          if !matches_exact_structure(&items[start_idx + i], pattern_item) {
            all_match = false;
            break;
          }
        }
        if all_match {
          return true;
        }
      }
      false
    }

    _ => false,
  }
}

/// Check if node exactly matches pattern structure
fn matches_exact_structure(node: &Cirru, pattern: &Cirru) -> bool {
  match (node, pattern) {
    (Cirru::Leaf(s1), Cirru::Leaf(s2)) => s1.as_ref() == s2.as_ref(),
    (Cirru::List(items1), Cirru::List(items2)) => {
      items1.len() == items2.len() && items1.iter().zip(items2.iter()).all(|(n1, n2)| matches_exact_structure(n1, n2))
    }
    _ => false,
  }
}
