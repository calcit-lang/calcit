//! Libs subcommand handler
//!
//! Handles: cr libs - fetches available Calcit libraries from registry

use calcit::cli_args::{LibsCommand, LibsSubcommand};
use colored::Colorize;
use serde::Deserialize;

/// Library entry from the registry
#[derive(Debug, Clone, Deserialize)]
pub struct LibraryEntry {
  #[serde(rename = ":package-name")]
  pub package_name: String,
  #[serde(rename = ":repository")]
  pub repository: String,
  #[serde(rename = ":category", default)]
  pub category: EdnSet,
  #[serde(rename = ":description", default)]
  pub description: Option<String>,
}

/// EDN Set representation (serialized as {"__edn_set": [...]})
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EdnSet {
  #[serde(rename = "__edn_set", default)]
  pub items: Vec<EdnTag>,
}

/// EDN Tag representation (serialized as {"__edn_tag": "..."})
#[derive(Debug, Clone, Deserialize)]
pub struct EdnTag {
  #[serde(rename = "__edn_tag")]
  pub tag: String,
}

impl EdnSet {
  pub fn to_strings(&self) -> Vec<String> {
    self.items.iter().map(|t| t.tag.clone()).collect()
  }
}

/// Library registry data
#[derive(Debug, Clone, Deserialize)]
pub struct LibraryRegistry {
  #[serde(rename = ":description")]
  pub description: String,
  #[serde(rename = ":libraries")]
  pub libraries: Vec<LibraryEntry>,
}

pub fn handle_libs_command(cmd: &LibsCommand) -> Result<(), String> {
  match &cmd.subcommand {
    None => handle_list_libs(),
    Some(LibsSubcommand::Readme(opts)) => handle_readme(&opts.package, opts.file.as_deref()),
    Some(LibsSubcommand::Search(opts)) => handle_search(&opts.keyword),
    Some(LibsSubcommand::ScanMd(opts)) => handle_scan_md(&opts.module),
  }
}

fn fetch_registry() -> Result<LibraryRegistry, String> {
  let url = "https://libs.calcit-lang.org/base.cirru";

  let client = reqwest::blocking::Client::new();

  let response = client
    .get(url)
    .send()
    .map_err(|e| format!("Failed to connect to library registry: {e}"))?;

  if !response.status().is_success() {
    return Err(format!("Failed to fetch libraries: HTTP status {}", response.status()));
  }

  let text = response.text().map_err(|e| format!("Failed to read response text: {e}"))?;

  // Parse Cirru EDN format
  let edn = cirru_edn::parse(&text).map_err(|e| format!("Failed to parse Cirru EDN: {e}"))?;

  // Convert to JSON then deserialize to struct
  let json_value = serde_json::to_value(&edn).map_err(|e| format!("Failed to convert EDN to JSON: {e}"))?;

  serde_json::from_value(json_value).map_err(|e| format!("Failed to parse library registry: {e}"))
}

fn handle_list_libs() -> Result<(), String> {
  println!("{}", "Fetching Calcit libraries from registry...".dimmed());

  let registry = fetch_registry()?;

  println!("\n{}", "Available Calcit Libraries:".bold());
  println!("{}", "=".repeat(60).dimmed());
  println!("{}", registry.description.dimmed());
  println!();

  for lib in &registry.libraries {
    println!("{}", lib.package_name.cyan().bold());
    println!("  {}: {}", "repo".dimmed(), lib.repository);

    let categories = lib.category.to_strings();
    if !categories.is_empty() {
      println!("  {}: {}", "category".dimmed(), categories.join(", "));
    }

    if let Some(desc) = &lib.description {
      println!("  {}: {}", "desc".dimmed(), desc);
    }

    println!();
  }

  println!("{}", format!("Total: {} libraries", registry.libraries.len()).dimmed());
  println!("\n{}", "Use 'cr libs readme <package>' to view library README.".dimmed());
  println!("{}", "Use 'cr libs search <keyword>' to search libraries.".dimmed());
  println!("{}", "Use 'caps' command to install libraries.".dimmed());

  Ok(())
}

fn handle_readme(package: &str, file: Option<&str>) -> Result<(), String> {
  let file_name = file.unwrap_or("README.md");
  println!("{}", format!("Looking for {file_name} in '{package}'...").dimmed());

  // Try local directory first: ~/.config/calcit/modules/<package>/<file>
  let home_dir = std::env::var("HOME").map_err(|_| "Failed to get HOME directory".to_string())?;
  let local_path = format!("{home_dir}/.config/calcit/modules/{package}/{file_name}");

  if let Ok(content) = std::fs::read_to_string(&local_path) {
    // Print library info header
    println!("\n{}", "═".repeat(60).dimmed());
    println!("{} {}", "Package:".bold(), package.cyan().bold());
    println!("{} {}", "Source:".bold(), "Local".green());
    println!("{} {}", "Path:".bold(), local_path.dimmed());
    println!("{}", "═".repeat(60).dimmed());
    println!();

    // Print file content
    println!("{content}");
    return Ok(());
  }

  // If not found locally, try fetching from GitHub
  println!("{}", "Not found locally, fetching from GitHub...".to_string().dimmed());

  let registry = fetch_registry()?;

  // Find the library
  let lib = registry
    .libraries
    .iter()
    .find(|l| l.package_name == package)
    .ok_or_else(|| format!("Package '{package}' not found in registry"))?;

  // Convert GitHub URL to raw file URL
  let base_url = github_to_raw_base(&lib.repository)?;

  let client = reqwest::blocking::Client::builder()
    .user_agent("calcit-cli")
    .build()
    .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

  // Try main branch first, then master
  let content = fetch_file_content(&client, &base_url, "main", file_name)
    .or_else(|_| fetch_file_content(&client, &base_url, "master", file_name))?;

  // Print library info header
  println!("\n{}", "═".repeat(60).dimmed());
  println!("{} {}", "Package:".bold(), lib.package_name.cyan().bold());
  println!("{} {}", "Repository:".bold(), lib.repository);
  if let Some(desc) = &lib.description {
    println!("{} {}", "Description:".bold(), desc);
  }
  println!("{} {}", "File:".bold(), file_name);
  println!("{}", "═".repeat(60).dimmed());
  println!();

  // Print file content
  println!("{content}");

  Ok(())
}

fn github_to_raw_base(repo_url: &str) -> Result<String, String> {
  // Parse: https://github.com/owner/repo -> https://raw.githubusercontent.com/owner/repo
  if !repo_url.starts_with("https://github.com/") {
    return Err(format!("Unsupported repository URL format: {repo_url}"));
  }

  let path = repo_url.trim_start_matches("https://github.com/").trim_end_matches('/');

  Ok(format!("https://raw.githubusercontent.com/{path}"))
}

fn fetch_file_content(client: &reqwest::blocking::Client, base_url: &str, branch: &str, file_name: &str) -> Result<String, String> {
  let url = format!("{base_url}/{branch}/{file_name}");

  let response = client.get(&url).send().map_err(|e| format!("Failed to fetch file: {e}"))?;

  if !response.status().is_success() {
    return Err(format!("File not found at {} (HTTP {})", url, response.status()));
  }

  response.text().map_err(|e| format!("Failed to read file: {e}"))
}

fn handle_search(keyword: &str) -> Result<(), String> {
  println!("{}", format!("Searching for '{keyword}'...").dimmed());

  let registry = fetch_registry()?;

  let keyword_lower = keyword.to_lowercase();

  let results: Vec<&LibraryEntry> = registry
    .libraries
    .iter()
    .filter(|lib| {
      lib.package_name.to_lowercase().contains(&keyword_lower)
        || lib.description.as_ref().is_some_and(|d| d.to_lowercase().contains(&keyword_lower))
        || lib.category.to_strings().iter().any(|c| c.to_lowercase().contains(&keyword_lower))
    })
    .collect();

  if results.is_empty() {
    println!("\n{}", format!("No libraries found matching '{keyword}'").yellow());
    return Ok(());
  }

  println!("\n{}", format!("Found {} libraries matching '{}':", results.len(), keyword).bold());
  println!("{}", "=".repeat(60).dimmed());

  for lib in results {
    println!("{}", lib.package_name.cyan().bold());
    println!("  {}: {}", "repo".dimmed(), lib.repository);

    let categories = lib.category.to_strings();
    if !categories.is_empty() {
      println!("  {}: {}", "category".dimmed(), categories.join(", "));
    }

    if let Some(desc) = &lib.description {
      println!("  {}: {}", "desc".dimmed(), desc);
    }

    println!();
  }

  println!("{}", "Use 'cr libs readme <package>' to view library README.".dimmed());

  Ok(())
}

fn handle_scan_md(module: &str) -> Result<(), String> {
  let home_dir = std::env::var("HOME").map_err(|_| "Failed to get HOME directory".to_string())?;
  let module_path = format!("{home_dir}/.config/calcit/modules/{module}");

  // Check if directory exists
  if !std::path::Path::new(&module_path).exists() {
    return Err(format!("Module directory not found: {module_path}"));
  }

  println!("{}", format!("Scanning markdown files in '{module}'...").cyan().bold());
  println!("{}: {}", "Path".dimmed(), module_path);
  println!();

  // Recursively scan for .md files
  let mut md_files = Vec::new();
  scan_directory(&module_path, &module_path, &mut md_files)?;

  if md_files.is_empty() {
    println!("{}", "No markdown files found.".yellow());
    return Ok(());
  }

  // Sort files for consistent output
  md_files.sort();

  println!("{}", format!("Found {} markdown files:", md_files.len()).bold());
  println!("{}", "=".repeat(60).dimmed());

  for file in &md_files {
    println!("  {}", file.cyan());
  }

  println!();
  println!(
    "{}",
    format!("Use 'cr libs readme {module} -f <file>' to read a specific file").dimmed()
  );

  Ok(())
}

fn scan_directory(base_path: &str, current_path: &str, results: &mut Vec<String>) -> Result<(), String> {
  let entries = std::fs::read_dir(current_path).map_err(|e| format!("Failed to read directory {current_path}: {e}"))?;

  for entry in entries {
    let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
    let path = entry.path();

    if path.is_file() {
      if let Some(ext) = path.extension() {
        if ext == "md" {
          // Get relative path from base
          let relative_path = path
            .strip_prefix(base_path)
            .map_err(|e| format!("Failed to get relative path: {e}"))?
            .to_string_lossy()
            .to_string();
          results.push(relative_path);
        }
      }
    } else if path.is_dir() {
      // Skip hidden directories
      if let Some(dir_name) = path.file_name() {
        if !dir_name.to_string_lossy().starts_with('.') {
          scan_directory(base_path, &path.to_string_lossy(), results)?;
        }
      }
    }
  }

  Ok(())
}
