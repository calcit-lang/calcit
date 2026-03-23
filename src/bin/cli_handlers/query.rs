//! Query subcommand handlers
//!
//! Handles: cr query ns, defs, def, at, peek, examples, find, usages, pkg, config, error, modules

use super::chunk_display::{ChunkDisplayOptions, ChunkedDisplay, maybe_chunk_node};
use super::common::{format_path, format_path_bracketed, parse_path};
use super::tips::{TipPriority, Tips, command_guidance_enabled, tip_prefer_oneliner_json, tip_query_defs_list, tip_query_ns_list};
use calcit::CalcitTypeAnnotation;
use calcit::cli_args::{QueryCommand, QueryDefCommand, QuerySubcommand};
use calcit::load_core_snapshot;
use calcit::snapshot;
use calcit::util::string::strip_shebang;
use cirru_parser::Cirru;
use colored::Colorize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use super::edit::navigate_to_path;

/// Type alias for search results: (namespace, definition, matches)
type SearchResults = Vec<(String, String, Vec<(Vec<usize>, Cirru)>)>;

/// Type alias for reference results: (namespace, definition, context, coordinate-path, source-label)
type RefResults = Vec<(String, String, String, Vec<Vec<usize>>, &'static str)>;

struct SearchCommonOpts<'a> {
  filter: Option<&'a str>,
  loose: bool,
  max_depth: usize,
  entry: Option<&'a str>,
  detail_offset: usize,
}

const DETAILED_RESULTS_WINDOW: usize = 3;

fn detailed_window(detail_offset: usize, total: usize) -> (usize, usize) {
  if total == 0 {
    return (0, 0);
  }
  let start = detail_offset.min(total.saturating_sub(1));
  let end = (start + DETAILED_RESULTS_WINDOW).min(total);
  (start, end)
}

fn print_detail_window_hint(total: usize, detail_offset: usize, subject: &str) {
  if total > DETAILED_RESULTS_WINDOW {
    let (start, end) = detailed_window(detail_offset, total);
    println!(
      "{}",
      format!("Detail window for {subject}: [{start}, {end}) (detail-offset={detail_offset}), other entries are compressed.").dimmed()
    );
  }
}

fn in_detail_window(index: usize, total: usize, detail_offset: usize) -> bool {
  if total <= DETAILED_RESULTS_WINDOW {
    return true;
  }
  let (start, end) = detailed_window(detail_offset, total);
  index >= start && index < end
}

fn preview_node_oneline(node: &Cirru, max_len: usize) -> (String, bool) {
  let text = match node {
    Cirru::Leaf(s) => s.to_string(),
    _ => node.format_one_liner().unwrap_or_default(),
  };
  if text.is_empty() {
    return ("(matched)".to_string(), false);
  }
  if text.len() > max_len {
    (text[..max_len].to_string(), true)
  } else {
    (text, false)
  }
}

fn is_token_delimiter(ch: Option<char>) -> bool {
  match ch {
    None => true,
    Some(c) => c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | '$' | ','),
  }
}

fn highlight_target_text(text: &str, target: Option<&str>, loose: bool) -> String {
  let Some(target) = target else {
    return text.to_string();
  };
  if target.is_empty() || !text.contains(target) {
    return text.to_string();
  }

  if loose {
    return text.replace(target, &format!("{}", target.bright_yellow().bold()));
  }

  let mut highlighted = String::with_capacity(text.len());
  let mut last_index = 0;

  for (idx, _) in text.match_indices(target) {
    let prev_char = text[..idx].chars().next_back();
    let next_char = text[idx + target.len()..].chars().next();
    if is_token_delimiter(prev_char) && is_token_delimiter(next_char) {
      highlighted.push_str(&text[last_index..idx]);
      highlighted.push_str(&format!("{}", target.bright_yellow().bold()));
      last_index = idx + target.len();
    }
  }

  if last_index == 0 {
    text.to_string()
  } else {
    highlighted.push_str(&text[last_index..]);
    highlighted
  }
}

fn path_parent(path: &[usize]) -> Option<Vec<usize>> {
  if path.is_empty() {
    None
  } else {
    Some(path[..path.len() - 1].to_vec())
  }
}

fn get_node_at_path(code: &Cirru, path: &[usize]) -> Option<Cirru> {
  if path.is_empty() {
    return Some(code.clone());
  }
  let mut current = code;
  for &idx in path {
    match current {
      Cirru::List(items) => current = items.get(idx)?,
      Cirru::Leaf(_) => return None,
    }
  }
  Some(current.clone())
}

fn count_nodes_limited(node: &Cirru, limit: usize) -> usize {
  fn walk(node: &Cirru, acc: &mut usize, limit: usize) {
    if *acc >= limit {
      return;
    }
    *acc += 1;
    if let Cirru::List(items) = node {
      for item in items {
        if *acc >= limit {
          break;
        }
        walk(item, acc, limit);
      }
    }
  }
  let mut acc = 0;
  walk(node, &mut acc, limit);
  acc
}

fn can_show_parent_preview(expr_path: &[usize], parent_node: &Cirru) -> bool {
  if expr_path.len() > 8 {
    return false;
  }
  if let Cirru::List(items) = parent_node
    && items.len() > 8
  {
    return false;
  }
  count_nodes_limited(parent_node, 40) < 40
}

fn expression_and_parent_preview(
  code: &Cirru,
  match_path: &[usize],
  matched_node: &Cirru,
  highlight_target: Option<&str>,
  loose: bool,
) -> ((String, bool), Vec<(String, bool)>) {
  let expr_path = if matches!(matched_node, Cirru::Leaf(_)) {
    path_parent(match_path).unwrap_or_else(|| match_path.to_vec())
  } else {
    match_path.to_vec()
  };

  let expr_node = get_node_at_path(code, &expr_path).unwrap_or_else(|| matched_node.clone());
  let (expr_text, expr_truncated) = preview_node_oneline(&expr_node, 110);
  let expr_preview = (highlight_target_text(&expr_text, highlight_target, loose), expr_truncated);

  let mut parent_previews: Vec<(String, bool)> = Vec::new();
  let mut current_path = expr_path;

  for _ in 0..2 {
    let Some(parent_path) = path_parent(&current_path) else {
      break;
    };
    let Some(parent_node) = get_node_at_path(code, &parent_path) else {
      break;
    };

    if can_show_parent_preview(&parent_path, &parent_node) {
      let (preview_text, preview_truncated) = preview_node_oneline(&parent_node, 110);
      parent_previews.push((highlight_target_text(&preview_text, highlight_target, loose), preview_truncated));
    }

    current_path = parent_path;
  }

  (expr_preview, parent_previews)
}

/// Parse "namespace/definition" format into (namespace, definition)
/// Splits at the FIRST '/' so operator definitions like '/' and '/=' are handled correctly.
fn parse_target(target: &str) -> Result<(&str, &str), String> {
  target
    .split_once('/')
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
      handle_def(input_path, ns, def, opts)
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
      if opts.exact {
        handle_find(input_path, &opts.symbol, opts.deps, opts.detail_offset)
      } else {
        handle_fuzzy_search(input_path, &opts.symbol, opts.deps, opts.limit, opts.detail_offset)
      }
    }
    QuerySubcommand::Usages(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_usages(input_path, ns, def, opts.deps, opts.detail_offset)
    }
    QuerySubcommand::Search(opts) => {
      let common_opts = SearchCommonOpts {
        filter: opts.filter.as_deref(),
        loose: !opts.exact,
        max_depth: opts.max_depth,
        entry: opts.entry.as_deref(),
        detail_offset: opts.detail_offset,
      };
      handle_search_leaf(input_path, &opts.pattern, opts.start_path.as_deref(), &common_opts)
    }
    QuerySubcommand::SearchExpr(opts) => {
      let common_opts = SearchCommonOpts {
        filter: opts.filter.as_deref(),
        loose: !opts.exact,
        max_depth: opts.max_depth,
        entry: opts.entry.as_deref(),
        detail_offset: opts.detail_offset,
      };
      handle_search_expr(input_path, &opts.pattern, opts.json, &common_opts)
    }
    QuerySubcommand::Schema(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_schema(input_path, ns, def, opts.json)
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
  let data = cirru_edn::parse(&content).map_err(|e| {
    eprintln!("\nFailed to parse file '{}':", fullpath.display());
    eprintln!("{e}");
    format!("Failed to parse file '{}'", fullpath.display())
  })?;
  snapshot::load_snapshot_data(&data, &fullpath.display().to_string())
}

fn load_snapshot(input_path: &str) -> Result<snapshot::Snapshot, String> {
  load_snapshot_with_entry(input_path, None)
}

fn load_snapshot_with_entry(input_path: &str, entry: Option<&str>) -> Result<snapshot::Snapshot, String> {
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

  let mut modules_to_load = snapshot.configs.modules.clone();
  if let Some(entry_name) = entry {
    let entry_config = snapshot.entries.get(entry_name).ok_or_else(|| {
      let available = if snapshot.entries.is_empty() {
        "(none)".to_owned()
      } else {
        snapshot.entries.keys().cloned().collect::<Vec<_>>().join(", ")
      };
      format!("Entry '{entry_name}' not found. Available entries: {available}")
    })?;
    modules_to_load.extend(entry_config.modules.clone());
  }

  let mut seen_modules = HashSet::new();
  modules_to_load.retain(|module_path| seen_modules.insert(module_path.to_owned()));

  // Load modules (dependencies) silently
  let base_dir = Path::new(input_path).parent().unwrap_or(Path::new("."));
  let module_folder = dirs::home_dir()
    .map(|buf| buf.as_path().join(".config/calcit/modules/"))
    .unwrap_or_else(|| Path::new(".").to_owned());

  for module_path in &modules_to_load {
    match load_module_silent(module_path, base_dir, &module_folder) {
      Ok(module_snapshot) => {
        for (ns_name, file_data) in module_snapshot.files {
          if snapshot.files.contains_key(&ns_name) {
            return Err(format!("namespace `{ns_name}` already exists when loading module `{module_path}`"));
          }
          snapshot.files.insert(ns_name, file_data);
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

  let mut tips = Tips::new();
  tips.append(tip_query_ns_list(include_deps));
  tips.print();

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

  if command_guidance_enabled() {
    println!(
      "\n{}",
      format!("Tip: Use `cr query defs {namespace}` to list definitions.").dimmed()
    );
  }

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
    let schema_hint = if !matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Dynamic) {
      " [schema]"
    } else {
      ""
    };
    if !entry.doc.is_empty() {
      let doc_first_line = entry.doc.lines().next().unwrap_or("");
      let doc_display = if doc_first_line.len() > 50 {
        format!("{}...", &doc_first_line[..50])
      } else {
        doc_first_line.to_string()
      };
      println!("  {}{} - {}", def.green(), schema_hint.dimmed(), doc_display.dimmed());
    } else {
      println!("  {}{}", def.green(), schema_hint.dimmed());
    }
  }

  let mut tips = Tips::new();
  tips.append(tip_query_defs_list());
  tips.print();

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
    if command_guidance_enabled() {
      println!();
      println!("{}", "Next steps:".blue().bold());
      println!("  • Start watcher: {} or {}", "cr".cyan(), "cr js".cyan());
      println!("  • Run syntax check: {}", "cr --check-only".cyan());
    }
    return Ok(());
  }

  let metadata = fs::metadata(error_file).map_err(|e| format!("Failed to get metadata of error file: {e}"))?;
  if let Ok(modified) = metadata.modified() {
    if let Ok(elapsed) = modified.elapsed() {
      if elapsed.as_secs() > 10 {
        println!(
          "{}",
          format!("Warning: .calcit-error.cirru was modified {} seconds ago.", elapsed.as_secs()).yellow()
        );
        println!("{}", "It might be outdated, please recompile or check the watcher.".yellow());
        println!();
      }
    }
  }

  let content = fs::read_to_string(error_file).map_err(|e| format!("Failed to read error file: {e}"))?;

  if content.trim().is_empty() {
    println!("{}", "✓ Error file is empty (no recent errors).".green());
    println!();
    println!("{}", "Your code compiled successfully!".dimmed());
    println!(
      "{}",
      "Note: this only reflects recent Calcit parsing/preprocess/runtime status; still validate browser rendering, CSS values, and external side effects separately."
        .dimmed()
    );
  } else {
    println!("{}", "Last error stack trace:".bold().red());
    println!("{content}");
    if command_guidance_enabled() {
      println!();
      println!("{}", "Next steps to fix:".blue().bold());
      println!("  • Search for error location: {} '<symbol>'", "cr query search".cyan());
      println!("  • View definition: {} '<ns/def>'", "cr query def".cyan());
      println!("  • Find usages: {} '<ns/def>'", "cr query usages".cyan());
      println!();
      println!("{}", "Tip: After fixing, watcher will recompile automatically (~300ms).".dimmed());
    }
    println!(
      "{}",
      "Note: even when this clears, non-Calcit issues like CSS strings, DOM behavior, and external integrations can still be wrong."
        .dimmed()
    );
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

  // Unified tips output
  let mut tips = Tips::new();
  tips.append(tip_query_ns_list(false));
  tips.print();

  Ok(())
}

fn render_chunked_display(display: &ChunkedDisplay) {
  println!("{}", "Chunked Cirru:".bold());
  println!(
    "{}",
    format!(
      "nodes: {}, branches: {}, leaves: {}, max depth: {}, fragments: {}",
      display.total.nodes,
      display.total.branches,
      display.total.leaves,
      display.total.max_depth,
      display.fragments.len()
    )
    .dimmed()
  );
  println!();

  for fragment in &display.fragments {
    println!("{} {}", fragment.id.cyan().bold(), format!("at {}", fragment.coord).dimmed());
    println!("{}", format!("nodes: {}, max depth: {}", fragment.nodes, fragment.depth).dimmed());
    for line in fragment.cirru.lines() {
      println!("  {line}");
    }
    println!();
  }
}

fn handle_def(input_path: &str, namespace: &str, definition: &str, opts: &QueryDefCommand) -> Result<(), String> {
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

  if let Ok(code_data) = calcit::data::cirru::code_to_calcit(&code_entry.code, namespace, definition, vec![]) {
    if let Some(summary) = CalcitTypeAnnotation::summarize_code(&code_data) {
      println!("{} {}", "Type:".bold(), summary);
    }
  }

  if !code_entry.doc.is_empty() {
    println!("{} {}", "Doc:".bold(), code_entry.doc);
  }

  if !code_entry.examples.is_empty() {
    println!("\n{} {}", "Examples:".bold(), code_entry.examples.len());
  }

  println!("\n{}", "Schema:".bold());
  if let CalcitTypeAnnotation::Fn(fn_annot) = code_entry.schema.as_ref() {
    let schema_str = match snapshot::schema_edn_to_cirru(&fn_annot.to_wrapped_schema_edn()) {
      Ok(c) => cirru_parser::format(std::slice::from_ref(&c), true.into()).unwrap_or_else(|_| "(failed to format)".to_string()),
      Err(e) => format!("(schema error: {e})"),
    };
    println!("{schema_str}");
  } else {
    println!("{}", "(none)".dimmed());
  }

  if !opts.raw {
    let chunk_options = ChunkDisplayOptions {
      trigger_nodes: opts.chunk_trigger_nodes,
      target_nodes: opts.chunk_target_nodes,
      max_nodes: opts.chunk_max_nodes,
      max_branches: 64,
    };
    if let Some(display) = maybe_chunk_node(&code_entry.code, &chunk_options)? {
      println!();
      render_chunked_display(&display);
    } else {
      println!("\n{}", "Cirru:".bold());
      let cirru_str =
        cirru_parser::format(&[code_entry.code.clone()], true.into()).unwrap_or_else(|_| "(failed to format)".to_string());
      println!("{cirru_str}");
    }
  } else {
    println!("\n{}", "Cirru:".bold());
    let cirru_str = cirru_parser::format(&[code_entry.code.clone()], true.into()).unwrap_or_else(|_| "(failed to format)".to_string());
    println!("{cirru_str}");
  }

  if opts.json {
    println!("\n{}", "JSON:".bold());
    let json = code_entry_to_json(code_entry);
    println!("{}", serde_json::to_string(&json).unwrap());
  }

  let mut tips = Tips::new();
  tips.add(format!(
    "Try `cr query search <leaf> -f '{namespace}/{definition}'` to find coordinates of a leaf node"
  ));
  tips.add(format!(
    "Use `cr tree show {namespace}/{definition} -p '0'` or `-p '0.1'` to explore tree for editing"
  ));
  if !code_entry.examples.is_empty() {
    tips.add(format!("Use `cr query examples {namespace}/{definition}` to view examples"));
  }
  tips.append(tip_prefer_oneliner_json(opts.json));
  tips.print();

  Ok(())
}

fn cirru_to_json(cirru: &Cirru) -> serde_json::Value {
  match cirru {
    Cirru::Leaf(s) => serde_json::Value::String(s.to_string()),
    Cirru::List(items) => serde_json::Value::Array(items.iter().map(cirru_to_json).collect()),
  }
}

fn code_entry_to_json(entry: &snapshot::CodeEntry) -> serde_json::Value {
  let schema_json = match entry.schema.as_ref() {
    CalcitTypeAnnotation::Fn(fn_annot) => snapshot::schema_edn_to_cirru(&fn_annot.to_schema_edn())
      .ok()
      .map(|c| cirru_to_json(&c)),
    _ => None,
  };
  serde_json::json!({
    "doc": entry.doc,
    "examples": entry.examples.iter().map(cirru_to_json).collect::<Vec<_>>(),
    "code": cirru_to_json(&entry.code),
    "schema": schema_json,
  })
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
    let mut tips = Tips::new();
    tips.add(format!("Use `cr edit examples {namespace}/{definition}` to add examples."));
    tips.print();
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

    let mut tips = Tips::new();
    tips.add(format!("Use `cr edit examples {namespace}/{definition}` to modify examples."));
    tips.print();
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

  if let CalcitTypeAnnotation::Fn(fn_annot) = code_entry.schema.as_ref() {
    let preview = match snapshot::schema_edn_to_cirru(&fn_annot.to_wrapped_schema_edn()) {
      Ok(c) => c.format_one_liner()?,
      Err(e) => format!("(schema error: {e})"),
    };
    let display = if preview.len() > 120 {
      format!("{}...", &preview[..120])
    } else {
      preview
    };
    println!("{} {}", "Schema:".bold(), display.dimmed());
  } else {
    println!("{} -", "Schema:".bold());
  }

  let mut tips = Tips::new();
  tips.add(format!("cr query def {namespace}/{definition}"));
  tips.add(format!("cr query examples {namespace}/{definition}"));
  tips.add(format!("cr query usages {namespace}/{definition}"));
  tips.add(format!("cr query schema {namespace}/{definition}"));
  tips.add(format!("cr edit doc {namespace}/{definition} '<doc>'"));
  tips.print();

  Ok(())
}

/// Show definition schema
fn handle_schema(input_path: &str, namespace: &str, definition: &str, json: bool) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let code_entry = file_data
    .defs
    .get(definition)
    .ok_or_else(|| format!("Definition '{definition}' not found in namespace '{namespace}'"))?;

  if json {
    let schema_edn: cirru_edn::Edn = match code_entry.schema.as_ref() {
      CalcitTypeAnnotation::Dynamic => cirru_edn::Edn::Nil,
      CalcitTypeAnnotation::Fn(fn_annot) => fn_annot.to_schema_edn(),
      _ => cirru_edn::Edn::Nil,
    };
    println!("{}", cirru_edn::format(&schema_edn, true)?);
    return Ok(());
  }

  println!("{} {}/{}", "Definition:".bold(), namespace.cyan(), definition.green());

  if let CalcitTypeAnnotation::Fn(fn_annot) = code_entry.schema.as_ref() {
    let cirru = snapshot::schema_edn_to_cirru(&fn_annot.to_wrapped_schema_edn())?;
    println!("{} {}", "Schema:".bold(), cirru.format_one_liner()?.dimmed());
  } else {
    println!("{} -", "Schema:".bold());
  }

  let mut tips = Tips::new();
  tips.add(format!("cr query peek {namespace}/{definition}"));
  tips.add(format!("cr edit schema {namespace}/{definition} -e '{{}} ...'"));
  tips.print();

  Ok(())
}

/// Find symbol across all namespaces
fn handle_find(input_path: &str, symbol: &str, include_deps: bool, detail_offset: usize) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let mut found_definitions: Vec<(String, String)> = vec![];
  let mut found_references: RefResults = vec![]; // (ns, def, context, coords, source)

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
          "code",
        ));
      }

      if let CalcitTypeAnnotation::Fn(fn_annot) = code_entry.schema.as_ref()
        && let Ok(schema) = snapshot::schema_edn_to_cirru(&fn_annot.to_schema_edn())
        && find_symbol_in_cirru(&schema, symbol)
      {
        let coords = find_symbol_coords(&schema, symbol);
        found_references.push((
          ns_name.clone(),
          def_name.clone(),
          get_symbol_context_cirru(&schema, symbol),
          coords,
          "schema",
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
    print_detail_window_hint(found_definitions.len(), detail_offset, "definitions");
    for (idx, (ns, def)) in found_definitions.iter().enumerate() {
      if in_detail_window(idx, found_definitions.len(), detail_offset) {
        println!("  {}/{}", ns.cyan(), def.green());
      } else {
        println!("  ⋯ {}/{}", ns.dimmed(), def.dimmed());
      }
    }
    println!();
  }

  // Print references (excluding the definition itself)
  let references: Vec<_> = found_references
    .iter()
    .filter(|(ns, def, _, _, _)| !found_definitions.iter().any(|(dns, ddef)| dns == ns && ddef == def))
    .collect();

  if !references.is_empty() {
    println!("{}", "Referenced in:".bold());
    print_detail_window_hint(references.len(), detail_offset, "references");
    for (idx, (ns, def, context, coords, source)) in references.iter().enumerate() {
      if !in_detail_window(idx, references.len(), detail_offset) {
        println!(
          "  ⋯ {}/{} [{}] ({} path{})",
          ns.dimmed(),
          def.dimmed(),
          source.dimmed(),
          coords.len(),
          if coords.len() == 1 { "" } else { "s" }
        );
        continue;
      }

      // Show main line
      if !context.is_empty() {
        println!("  {}/{} [{}]  {}", ns.cyan(), def, source.dimmed(), context.dimmed());
      } else {
        println!("  {}/{} [{}]", ns.cyan(), def, source.dimmed());
      }

      // Show coordinates on one line with "and" separator
      if !coords.is_empty() {
        let coords_parts: Vec<String> = coords
          .iter()
          .map(|path| {
            let coord_str = format_path(path);
            format!("[{coord_str}]")
          })
          .collect();
        println!("    {}", format!("at {}", coords_parts.join(" and ")).dimmed());
      }
    }
  }

  if found_definitions.is_empty() && references.is_empty() {
    println!("{}", "No matches found.".yellow());
    let mut tips = Tips::new();
    tips.add("Try `cr query ns` to see available namespaces.");
    tips.print();
  } else if !found_definitions.is_empty() {
    let (first_ns, first_def) = &found_definitions[0];
    let mut tips = Tips::new();
    tips.add(format!("Use `cr query peek {first_ns}/{first_def}` to see signature."));
    tips.print();
  }

  Ok(())
}

/// Find usages of a specific definition
fn handle_usages(input_path: &str, target_ns: &str, target_def: &str, include_deps: bool, detail_offset: usize) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  // Verify the target definition exists
  let _ = snapshot
    .files
    .get(target_ns)
    .ok_or_else(|| format!("Namespace '{target_ns}' not found"))?
    .defs
    .get(target_def)
    .ok_or_else(|| format!("Definition '{target_def}' not found in namespace '{target_ns}'"))?;

  let mut usages: RefResults = vec![]; // (ns, def, context, coords, source)

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
      let found_in_code = if imports_target || ns_name == target_ns {
        find_symbol_in_cirru(&code_entry.code, target_def)
      } else {
        // Check for qualified reference: target_ns/target_def
        let qualified = format!("{target_ns}/{target_def}");
        find_symbol_in_cirru(&code_entry.code, &qualified)
      };

      if found_in_code {
        let context = get_symbol_context_cirru(&code_entry.code, target_def);
        let coords = if imports_target || ns_name == target_ns {
          find_symbol_coords(&code_entry.code, target_def)
        } else {
          let qualified = format!("{target_ns}/{target_def}");
          find_symbol_coords(&code_entry.code, &qualified)
        };
        usages.push((ns_name.clone(), def_name.clone(), context, coords, "code"));
      }

      if let CalcitTypeAnnotation::Fn(fn_annot) = code_entry.schema.as_ref() {
        if let Ok(schema) = snapshot::schema_edn_to_cirru(&fn_annot.to_schema_edn()) {
          let found_in_schema = if imports_target || ns_name == target_ns {
            find_symbol_in_cirru(&schema, target_def)
          } else {
            let qualified = format!("{target_ns}/{target_def}");
            find_symbol_in_cirru(&schema, &qualified)
          };

          if found_in_schema {
            let context = get_symbol_context_cirru(&schema, target_def);
            let coords = if imports_target || ns_name == target_ns {
              find_symbol_coords(&schema, target_def)
            } else {
              let qualified = format!("{target_ns}/{target_def}");
              find_symbol_coords(&schema, &qualified)
            };
            usages.push((ns_name.clone(), def_name.clone(), context, coords, "schema"));
          }
        }
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
    print_detail_window_hint(usages.len(), detail_offset, "usages");
    for (idx, (ns, def, context, coords, source)) in usages.iter().enumerate() {
      if !in_detail_window(idx, usages.len(), detail_offset) {
        println!(
          "  ⋯ {}/{} [{}] ({} path{})",
          ns.dimmed(),
          def.dimmed(),
          source.dimmed(),
          coords.len(),
          if coords.len() == 1 { "" } else { "s" }
        );
        continue;
      }

      // Show main line
      if !context.is_empty() {
        println!("  {}/{} [{}]  {}", ns.cyan(), def.green(), source.dimmed(), context.dimmed());
      } else {
        println!("  {}/{} [{}]", ns.cyan(), def.green(), source.dimmed());
      }

      // Show coordinates on one line with "and" separator
      if !coords.is_empty() {
        let coords_parts: Vec<String> = coords
          .iter()
          .map(|path| {
            let coord_str = format_path(path);
            format!("[{coord_str}]")
          })
          .collect();
        println!("    {}", format!("at {}", coords_parts.join(" and ")).dimmed());
      }
    }
  }

  // Tip
  if !usages.is_empty() && command_guidance_enabled() {
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
fn handle_fuzzy_search(input_path: &str, pattern: &str, include_deps: bool, limit: usize, detail_offset: usize) -> Result<(), String> {
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
    if command_guidance_enabled() {
      println!(
        "\n{}",
        "Tip: Try a broader pattern, or add --deps to include core namespaces.".dimmed()
      );
    }
    return Ok(());
  }

  print_detail_window_hint(displayed.len(), detail_offset, "search results");

  for (idx, (ns, def, is_core)) in displayed.iter().enumerate() {
    if !in_detail_window(idx, displayed.len(), detail_offset) {
      println!("  ⋯ {}/{}", ns.dimmed(), def.dimmed());
      continue;
    }

    let qualified = format!("{}/{}", ns.cyan(), def.green());
    if *is_core {
      println!("  {} {}", qualified, "(core)".dimmed());
    } else {
      println!("  {qualified}");
    }
  }

  if total > limit {
    println!("  ⋯ {} more results...", total - limit);
  }

  if command_guidance_enabled() {
    println!("\n{}", "Tip: Use `query def <ns/def>` to view definition content.".dimmed());
  }

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
fn handle_search_leaf(input_path: &str, pattern: &str, start_path: Option<&str>, common_opts: &SearchCommonOpts) -> Result<(), String> {
  let snapshot = load_snapshot_with_entry(input_path, common_opts.entry)?;

  // Parse start_path if provided
  let parsed_start_path: Option<Vec<usize>> = if let Some(path_str) = start_path {
    if path_str.is_empty() {
      Some(vec![])
    } else {
      Some(parse_path(path_str).map_err(|e| format!("Invalid start path '{path_str}': {e}"))?)
    }
  } else {
    None
  };

  println!("{} Searching for:", "Search:".bold());
  if common_opts.loose {
    println!("  {} (contains)", pattern.yellow());
  } else {
    println!("  {} (exact)", pattern.yellow());
  }

  if let Some(filter_str) = common_opts.filter {
    println!("  {} {}", "Filter:".dimmed(), filter_str.cyan());
  } else {
    println!("  {} {}", "Scope:".dimmed(), "entire project".cyan());
  }
  if let Some(entry_name) = common_opts.entry {
    println!("  {} {}", "Entry:".dimmed(), entry_name.cyan());
  }

  if let Some(ref path) = parsed_start_path {
    let path_display = format_path_bracketed(path);
    println!("  {} {}", "Start path:".dimmed(), path_display.cyan());
  }
  println!();

  let mut all_results: SearchResults = Vec::new();

  // Parse filter to determine scope
  let (filter_ns, filter_def) = if let Some(f) = common_opts.filter {
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
      let results = search_leaf_nodes(&search_root, pattern, common_opts.loose, common_opts.max_depth, base_path);

      if !results.is_empty() {
        all_results.push((ns.clone(), def_name.clone(), results));
      }
    }
  }

  // Print results grouped by namespace/definition
  if all_results.is_empty() {
    println!("{}", "No matches found.".yellow());
  } else {
    all_results.sort_by(|a, b| b.2.len().cmp(&a.2.len()).then_with(|| a.0.cmp(&b.0)).then_with(|| a.1.cmp(&b.1)));

    let total_matches: usize = all_results.iter().map(|(_, _, results)| results.len()).sum();
    println!(
      "{} {} match(es) found in {} definition(s):\n",
      "Results:".bold().green(),
      total_matches,
      all_results.len()
    );

    for (ns, def_name, results) in &all_results {
      println!("{} {}/{} ({} matches)", "●".cyan(), ns.dimmed(), def_name.green(), results.len());
      print_detail_window_hint(results.len(), common_opts.detail_offset, "matches");

      // Load code_entry to print results
      if let Some(file_data) = snapshot.files.get(ns) {
        if let Some(code_entry) = file_data.defs.get(def_name) {
          let total = results.len();
          let (start, end) = detailed_window(common_opts.detail_offset, total);
          let detailed_count = end.saturating_sub(start);
          let compressed_count = total.saturating_sub(detailed_count);

          for (path, node) in results.iter().skip(start).take(detailed_count) {
            if path.is_empty() {
              let (content, truncated) = preview_node_oneline(&code_entry.code, 110);
              if truncated {
                println!("    {} {} ⟪…⟫", "(root)".cyan(), content.dimmed());
              } else {
                println!("    {} {}", "(root)".cyan(), content.dimmed());
              }
            } else {
              let path_str = format!("[{}]", format_path(path));
              let ((expr_preview, expr_truncated), parent_previews) =
                expression_and_parent_preview(&code_entry.code, path, node, Some(pattern), common_opts.loose);
              let (display_preview, display_truncated) = parent_previews
                .first()
                .map(|(text, truncated)| (text.as_str(), *truncated))
                .unwrap_or((expr_preview.as_str(), expr_truncated));
              if display_truncated {
                println!("    {} {} ⟪…⟫", path_str.cyan(), display_preview);
              } else {
                println!("    {} {}", path_str.cyan(), display_preview);
              }
            }
          }

          if compressed_count > 0 {
            println!("    {}", format!("{compressed_count} matches compressed outside window").dimmed());
          }
        }
      }
      println!();
    }

    let mut tips = Tips::new();
    if total_matches > 10 && common_opts.loose {
      tips.add_with_priority(
        TipPriority::High,
        format!(
          "Many matches ({total_matches}); add {} to show exact matches only",
          "--exact".yellow()
        ),
      );
    }
    tips.print();
  }

  Ok(())
}

/// Search for structural expressions across project or in filtered scope
fn handle_search_expr(input_path: &str, pattern: &str, json: bool, common_opts: &SearchCommonOpts) -> Result<(), String> {
  let snapshot = load_snapshot_with_entry(input_path, common_opts.entry)?;

  let pattern_node = if json {
    let json_val: serde_json::Value = serde_json::from_str(pattern).map_err(|e| format!("Failed to parse JSON pattern: {e}"))?;
    json_to_cirru(&json_val)?
  } else {
    cirru_parser::parse(pattern)
      .map_err(|e| format!("Failed to parse Cirru pattern: {e}"))?
      .first()
      .ok_or("Pattern is empty")?
      .clone()
  };

  println!("{} Searching for pattern:", "Search:".bold());

  let pattern_display = pattern_node.format_one_liner().unwrap_or_default();
  let highlight_target: Option<&str> = match &pattern_node {
    Cirru::Leaf(s) => Some(s.as_ref()),
    _ => None,
  };

  if common_opts.loose {
    println!("  {} (substring match for leaf patterns)", pattern_display.yellow());
  } else {
    println!("  {} (exact match)", pattern_display.yellow());
  }

  if let Some(filter_str) = common_opts.filter {
    println!("  {} {}", "Filter:".dimmed(), filter_str.cyan());
  } else {
    println!("  {} {}", "Scope:".dimmed(), "entire project".cyan());
  }
  if let Some(entry_name) = common_opts.entry {
    println!("  {} {}", "Entry:".dimmed(), entry_name.cyan());
  }
  println!();

  let mut all_results: SearchResults = Vec::new();

  let (filter_ns, filter_def) = if let Some(f) = common_opts.filter {
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

  for (ns, file_data) in &snapshot.files {
    if let Some(filter_namespace) = filter_ns
      && ns != filter_namespace
    {
      continue;
    }

    for (def_name, code_entry) in &file_data.defs {
      if let Some(filter_definition) = filter_def
        && def_name != filter_definition
      {
        continue;
      }

      let results = search_expr_nodes(&code_entry.code, &pattern_node, common_opts.loose, common_opts.max_depth, &[]);
      if !results.is_empty() {
        all_results.push((ns.clone(), def_name.clone(), results));
      }
    }
  }

  if all_results.is_empty() {
    println!("{}", "No matches found.".yellow());
  } else {
    all_results.sort_by(|a, b| b.2.len().cmp(&a.2.len()).then_with(|| a.0.cmp(&b.0)).then_with(|| a.1.cmp(&b.1)));

    let total_matches: usize = all_results.iter().map(|(_, _, results)| results.len()).sum();
    println!(
      "{} {} match(es) found in {} definition(s):\n",
      "Results:".bold().green(),
      total_matches,
      all_results.len()
    );

    for (ns, def_name, results) in &all_results {
      println!("{} {}/{} ({} matches)", "●".cyan(), ns.dimmed(), def_name.green(), results.len());
      print_detail_window_hint(results.len(), common_opts.detail_offset, "matches");

      if let Some(file_data) = snapshot.files.get(ns)
        && let Some(code_entry) = file_data.defs.get(def_name)
      {
        let total = results.len();
        let (start, end) = detailed_window(common_opts.detail_offset, total);
        let detailed_count = end.saturating_sub(start);
        let compressed_count = total.saturating_sub(detailed_count);

        for (path, node) in results.iter().skip(start).take(detailed_count) {
          let path_str = format_path(path);
          if path.is_empty() {
            let (content, truncated) = preview_node_oneline(&code_entry.code, 110);
            if truncated {
              println!("    {} {} ⟪…⟫", "(root)".cyan(), content.dimmed());
            } else {
              println!("    {} {}", "(root)".cyan(), content.dimmed());
            }
          } else {
            let ((expr_preview, expr_truncated), parent_previews) =
              expression_and_parent_preview(&code_entry.code, path, node, highlight_target, common_opts.loose);
            let (display_preview, display_truncated) = parent_previews
              .first()
              .map(|(text, truncated)| (text.as_str(), *truncated))
              .unwrap_or((expr_preview.as_str(), expr_truncated));
            if display_truncated {
              println!("    {} {} ⟪…⟫", format!("[{path_str}]").cyan(), display_preview);
            } else {
              println!("    {} {}", format!("[{path_str}]").cyan(), display_preview);
            }
          }
        }

        if compressed_count > 0 {
          println!("    {}", format!("{compressed_count} matches compressed outside window").dimmed());
        }
      }

      println!();
    }

    let mut tips = Tips::new();
    if total_matches > 10 && common_opts.loose {
      tips.add_with_priority(
        TipPriority::High,
        format!(
          "Many matches ({total_matches}); add {} to show exact matches only",
          "--exact".yellow()
        ),
      );
    }
    tips.print();
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

fn search_expr_nodes(node: &Cirru, pattern: &Cirru, loose: bool, max_depth: usize, current_path: &[usize]) -> Vec<(Vec<usize>, Cirru)> {
  let mut results = Vec::new();

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

/// Check if node starts with pattern (prefix matching)
/// In loose mode, pattern must match from the beginning of the list
fn contains_pattern(node: &Cirru, pattern: &Cirru) -> bool {
  match (node, pattern) {
    // Leaf nodes: check string containment
    (Cirru::Leaf(s), Cirru::Leaf(p)) => s.to_lowercase().contains(&p.as_ref().to_lowercase()),

    // List: pattern must match from the beginning (prefix match)
    (Cirru::List(items), Cirru::List(pattern_items)) => {
      if pattern_items.is_empty() {
        return true;
      }

      // Pattern must not be longer than the actual list
      if pattern_items.len() > items.len() {
        return false;
      }

      // Check if pattern matches from the beginning using prefix matching
      for (i, pattern_item) in pattern_items.iter().enumerate() {
        if !matches_prefix_structure(&items[i], pattern_item) {
          return false;
        }
      }
      true
    }

    _ => false,
  }
}

/// Check if node matches pattern as a prefix (allows node to be longer than pattern)
fn matches_prefix_structure(node: &Cirru, pattern: &Cirru) -> bool {
  match (node, pattern) {
    (Cirru::Leaf(s1), Cirru::Leaf(s2)) => s1.as_ref() == s2.as_ref(),
    (Cirru::List(items1), Cirru::List(items2)) => {
      // Pattern must not be longer than node
      if items2.len() > items1.len() {
        return false;
      }
      // Check if pattern matches the prefix of node
      items2
        .iter()
        .enumerate()
        .all(|(i, pattern_item)| matches_prefix_structure(&items1[i], pattern_item))
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
