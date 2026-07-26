//! Libs subcommand handler
//!
//! Handles: cr libs - fetches available Calcit libraries from registry

use calcit::cli_args::{LibsCommand, LibsSubcommand};
use colored::Colorize;
use serde::Deserialize;

use super::markdown_read::{RenderMarkdownOptions, render_markdown_sections};

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

struct ReadmeHeader<'a> {
  package: &'a str,
  source: Option<&'a str>,
  path: Option<&'a str>,
  repository: Option<&'a str>,
  description: Option<&'a str>,
  file: Option<&'a str>,
}

pub fn handle_libs_command(cmd: &LibsCommand) -> Result<(), String> {
  match &cmd.subcommand {
    None => handle_list_libs(),
    Some(LibsSubcommand::Readme(opts)) => handle_readme(
      &opts.package,
      opts.file.as_deref(),
      &opts.headings,
      !opts.no_subheadings,
      opts.full,
      opts.with_lines,
    ),
    Some(LibsSubcommand::Search(opts)) => handle_search(&opts.keyword),
    Some(LibsSubcommand::ScanMd(opts)) => handle_scan_md(&opts.module),
  }
}

fn fetch_registry() -> Result<LibraryRegistry, String> {
  let url = "https://libs.calcit-lang.org/base.cirru";

  let response = ureq::get(url)
    .call()
    .map_err(|e| format!("Failed to connect to library registry: {e}"))?;

  let text = response
    .into_body()
    .read_to_string()
    .map_err(|e| format!("Failed to read response text: {e}"))?;

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
  println!(
    "\n{}",
    "Use 'cr docs remote-libs readme <package>' to view library README.".dimmed()
  );
  println!("{}", "Use 'cr docs remote-libs search <keyword>' to search libraries.".dimmed());
  println!("{}", "Use 'caps' command to install libraries.".dimmed());

  Ok(())
}

fn print_readme_header(header: ReadmeHeader<'_>) {
  println!("\n{} {}", "Package:".bold(), header.package.cyan().bold());

  if let Some(source) = header.source {
    println!("{} {}", "Source:".bold(), source.green());
  }
  if let Some(path) = header.path {
    println!("{} {}", "Path:".bold(), path.dimmed());
  }
  if let Some(repository) = header.repository {
    println!("{} {}", "Repository:".bold(), repository);
  }
  if let Some(description) = header.description {
    println!("{} {}", "Description:".bold(), description);
  }
  if let Some(file) = header.file {
    println!("{} {}", "File:".bold(), file);
  }

  println!();
}

fn handle_readme(
  package: &str,
  file: Option<&str>,
  heading_queries: &[String],
  include_subheadings: bool,
  full: bool,
  with_lines: bool,
) -> Result<(), String> {
  let file_name = file.unwrap_or("README.md");
  println!("{}", format!("Looking for {file_name} in '{package}'...").dimmed());

  // Try local directory first: ~/.config/calcit/modules/<package>/<file>
  let home_dir = std::env::var("HOME").map_err(|_| "Failed to get HOME directory".to_string())?;
  let local_path = format!("{home_dir}/.config/calcit/modules/{package}/{file_name}");

  let render_readme = |content: &str| -> Result<(), String> {
    let no_match_error =
      format!("No heading matched in {file_name}. Use 'cr docs remote-libs readme {package} --file {file_name}' to list headings.");
    render_markdown_sections(
      content,
      heading_queries,
      RenderMarkdownOptions {
        include_subheadings,
        full,
        with_lines,
        command_prefix: "cr docs remote-libs readme <package>",
        with_file_option: true,
        no_headings_message: "No Markdown headings found, printing full file.",
        print_full_when_no_headings: true,
        no_match_error: &no_match_error,
      },
    )
  };

  if let Ok(content) = std::fs::read_to_string(&local_path) {
    print_readme_header(ReadmeHeader {
      package,
      source: Some("Local"),
      path: Some(&local_path),
      repository: None,
      description: None,
      file: None,
    });

    return render_readme(&content);
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

  let agent = ureq::Agent::config_builder().user_agent("calcit-cli").build().new_agent();

  // Try main branch first, then master
  let content =
    fetch_file_content(&agent, &base_url, "main", file_name).or_else(|_| fetch_file_content(&agent, &base_url, "master", file_name))?;

  print_readme_header(ReadmeHeader {
    package: &lib.package_name,
    source: None,
    path: None,
    repository: Some(&lib.repository),
    description: lib.description.as_deref(),
    file: Some(file_name),
  });

  render_readme(&content)
}

fn github_to_raw_base(repo_url: &str) -> Result<String, String> {
  // Parse: https://github.com/owner/repo -> https://raw.githubusercontent.com/owner/repo
  if !repo_url.starts_with("https://github.com/") {
    return Err(format!("Unsupported repository URL format: {repo_url}"));
  }

  let path = repo_url.trim_start_matches("https://github.com/").trim_end_matches('/');

  Ok(format!("https://raw.githubusercontent.com/{path}"))
}

fn fetch_file_content(agent: &ureq::Agent, base_url: &str, branch: &str, file_name: &str) -> Result<String, String> {
  let url = format!("{base_url}/{branch}/{file_name}");

  let response = agent.get(&url).call().map_err(|e| format!("Failed to fetch file: {e}"))?;

  response
    .into_body()
    .read_to_string()
    .map_err(|e| format!("Failed to read file: {e}"))
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

  println!("{}", "Use 'cr docs remote-libs readme <package>' to view library README.".dimmed());

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
    format!("Use 'cr docs remote-libs readme {module} --file <file>' to read a specific file").dimmed()
  );

  Ok(())
}

fn scan_directory(base_path: &str, current_path: &str, results: &mut Vec<String>) -> Result<(), String> {
  let entries = std::fs::read_dir(current_path).map_err(|e| format!("Failed to read directory {current_path}: {e}"))?;

  for entry in entries {
    let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
    let path = entry.path();

    if path.is_file() {
      if let Some(ext) = path.extension()
        && ext == "md"
      {
        // Get relative path from base
        let relative_path = path
          .strip_prefix(base_path)
          .map_err(|e| format!("Failed to get relative path: {e}"))?
          .to_string_lossy()
          .to_string();
        results.push(relative_path);
      }
    } else if path.is_dir() {
      // Skip hidden directories
      if let Some(dir_name) = path.file_name()
        && !dir_name.to_string_lossy().starts_with('.')
      {
        scan_directory(base_path, &path.to_string_lossy(), results)?;
      }
    }
  }

  Ok(())
}
