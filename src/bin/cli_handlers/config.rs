//! Config command handler — top-level `cr config` shortcut
//!
//! Consolidates: cr query config, cr query modules, cr edit config, cr edit add-module, cr edit rm-module

use calcit::cli_args::{
  ConfigAddModuleCommand, ConfigCommand, ConfigRmModuleCommand, ConfigSetCommand, ConfigSubcommand, ConfigVersionCommand,
};
use calcit::snapshot;
use calcit::util::string::strip_shebang;
use colored::Colorize;
use std::fs;
use std::path::Path;

use super::edit::{bump_semver_value, load_snapshot, parse_semver_value, save_snapshot};

fn load_snapshot_for_display(input_path: &str) -> Result<snapshot::Snapshot, String> {
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
  snapshot::load_snapshot_data(&data, input_path)
}

pub fn handle_config_command(cmd: &ConfigCommand, snapshot_file: &str) -> Result<(), String> {
  match &cmd.subcommand {
    ConfigSubcommand::Show(_) => handle_show(snapshot_file),
    ConfigSubcommand::Modules(_) => handle_modules(snapshot_file),
    ConfigSubcommand::Version(opts) => handle_version(opts, snapshot_file),
    ConfigSubcommand::Set(opts) => handle_set(opts, snapshot_file),
    ConfigSubcommand::AddModule(opts) => handle_add_module(opts, snapshot_file),
    ConfigSubcommand::RmModule(opts) => handle_rm_module(opts, snapshot_file),
  }
}

fn handle_show(input_path: &str) -> Result<(), String> {
  let snapshot = load_snapshot_for_display(input_path)?;

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

fn handle_modules(input_path: &str) -> Result<(), String> {
  let snapshot = load_snapshot_for_display(input_path)?;

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

  Ok(())
}

fn load_module_silent(module_path: &str, base_dir: &Path, module_folder: &Path) -> Result<snapshot::Snapshot, String> {
  let candidates = [
    base_dir.join(module_path).join("calcit.cirru"),
    base_dir.join(module_path).join("compact.cirru"),
    module_folder.join(module_path).join("calcit.cirru"),
    module_folder.join(module_path).join("compact.cirru"),
  ];

  for candidate in &candidates {
    if candidate.exists() {
      let mut content = fs::read_to_string(candidate).map_err(|e| format!("Failed to read: {e}"))?;
      strip_shebang(&mut content);
      let data = cirru_edn::parse(&content).map_err(|e| format!("Failed to parse: {e}"))?;
      return snapshot::load_snapshot_data(&data, &candidate.to_string_lossy());
    }
  }

  Err(format!("Module not found: {module_path}"))
}

fn handle_version(opts: &ConfigVersionCommand, snapshot_file: &str) -> Result<(), String> {
  match &opts.value {
    None => {
      // Show current version
      let snapshot = load_snapshot_for_display(snapshot_file)?;
      println!("{}", snapshot.configs.version);
      Ok(())
    }
    Some(v) if matches!(v.as_str(), "patch" | "minor" | "major") => {
      let mut snapshot = load_snapshot(snapshot_file)?;
      let previous = snapshot.configs.version.clone();
      let next = bump_semver_value(&previous, v)?;
      snapshot.configs.version = next.clone();
      save_snapshot(&snapshot, snapshot_file)?;
      println!("{} Bumped version: {} → {}", "✓".green(), previous.yellow(), next.green());
      Ok(())
    }
    Some(v) => {
      parse_semver_value(v)?;
      let mut snapshot = load_snapshot(snapshot_file)?;
      snapshot.configs.version = v.clone();
      save_snapshot(&snapshot, snapshot_file)?;
      println!("{} Set version to {}", "✓".green(), v.green());
      Ok(())
    }
  }
}

fn handle_set(opts: &ConfigSetCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  let message = match opts.key.as_str() {
    "init-fn" | "init_fn" => {
      snapshot.configs.init_fn = opts.value.clone();
      format!("{} Set config '{}' = '{}'", "✓".green(), opts.key.cyan(), opts.value)
    }
    "reload-fn" | "reload_fn" => {
      snapshot.configs.reload_fn = opts.value.clone();
      format!("{} Set config '{}' = '{}'", "✓".green(), opts.key.cyan(), opts.value)
    }
    "version" => {
      if matches!(opts.value.as_str(), "patch" | "minor" | "major") {
        let previous = snapshot.configs.version.clone();
        let next = bump_semver_value(&previous, &opts.value)?;
        snapshot.configs.version = next.clone();
        format!(
          "{} Bumped config '{}' from '{}' to '{}'",
          "✓".green(),
          "version".cyan(),
          previous,
          next
        )
      } else {
        parse_semver_value(&opts.value)?;
        snapshot.configs.version = opts.value.clone();
        format!("{} Set config '{}' = '{}'", "✓".green(), opts.key.cyan(), opts.value)
      }
    }
    _ => {
      return Err(format!(
        "Unknown config key '{}'. Valid keys: init-fn, reload-fn, version (accepts semver string or patch|minor|major)",
        opts.key
      ));
    }
  };

  save_snapshot(&snapshot, snapshot_file)?;
  println!("{message}");
  Ok(())
}

fn handle_add_module(opts: &ConfigAddModuleCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  if snapshot.configs.modules.contains(&opts.module_path) {
    return Err(format!("Module '{}' already exists in configs", opts.module_path));
  }

  snapshot.configs.modules.push(opts.module_path.clone());
  save_snapshot(&snapshot, snapshot_file)?;

  println!("{} Added module '{}'", "✓".green(), opts.module_path.cyan());
  Ok(())
}

fn handle_rm_module(opts: &ConfigRmModuleCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  let original_len = snapshot.configs.modules.len();
  snapshot.configs.modules.retain(|m| m != &opts.module_path);

  if snapshot.configs.modules.len() == original_len {
    return Err(format!("Module '{}' not found in configs", opts.module_path));
  }

  save_snapshot(&snapshot, snapshot_file)?;
  println!("{} Removed module '{}'", "✓".green(), opts.module_path.cyan());
  Ok(())
}
