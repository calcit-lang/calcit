//! Query subcommand handlers
//!
//! Handles: cr query ls-ns, ls-defs, read-ns, pkg-name, configs, error, ls-modules

use calcit::cli_args::{QueryCommand, QuerySubcommand};
use calcit::snapshot;
use calcit::util::string::strip_shebang;
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
