//! Docs subcommand handlers
//!
//! Handles: cr docs api, ref, list-api, list-guide

use calcit::cli_args::{DocsCommand, DocsSubcommand};
use cirru_edn::Edn;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiDoc {
  name: String,
  desc: String,
  tags: HashSet<String>,
  snippets: Vec<Edn>,
}

#[derive(Debug, Clone)]
pub struct GuideDoc {
  filename: String,
  path: String,
  content: String,
}

pub fn handle_docs_command(cmd: &DocsCommand) -> Result<(), String> {
  match &cmd.subcommand {
    DocsSubcommand::Api(opts) => handle_api(&opts.query_type, opts.query.as_deref()),
    DocsSubcommand::Ref(opts) => handle_ref(&opts.query_type, opts.query.as_deref()),
    DocsSubcommand::ListApi(_) => handle_list_api(),
    DocsSubcommand::ListGuide(_) => handle_list_guide(),
  }
}

fn get_api_docs_dir() -> Result<std::path::PathBuf, String> {
  let home_dir = std::env::var("HOME").map_err(|_| "Unable to get HOME environment variable")?;
  let docs_dir = Path::new(&home_dir).join(".config/calcit/apis-repo/docs");

  if !docs_dir.exists() {
    return Err(format!(
      "API documentation directory not found: {docs_dir:?}\n\n\
       To set up API documentation, please run:\n\
       git clone https://github.com/calcit-lang/apis-repo.git ~/.config/calcit/apis-repo"
    ));
  }

  Ok(docs_dir)
}

fn get_guidebook_dir() -> Result<std::path::PathBuf, String> {
  let home_dir = std::env::var("HOME").map_err(|_| "Unable to get HOME environment variable")?;
  let docs_dir = Path::new(&home_dir).join(".config/calcit/guidebook-repo/docs");

  if !docs_dir.exists() {
    return Err(format!(
      "Guidebook documentation directory not found: {docs_dir:?}\n\n\
       To set up guidebook documentation, please run:\n\
       git clone https://github.com/calcit-lang/guidebook-repo.git ~/.config/calcit/guidebook-repo"
    ));
  }

  Ok(docs_dir)
}

fn load_api_docs() -> Result<HashMap<String, ApiDoc>, String> {
  let docs_dir = get_api_docs_dir()?;
  let mut api_docs = HashMap::new();

  for entry in fs::read_dir(&docs_dir).map_err(|e| format!("Failed to read directory: {e}"))? {
    let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
    let path = entry.path();

    if path.extension().and_then(|s| s.to_str()) == Some("cirru") {
      let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read file {path:?}: {e}"))?;
      let edn_data = cirru_edn::parse(&content).map_err(|e| format!("Failed to parse Cirru EDN {path:?}: {e}"))?;

      // Try to parse as Vec
      if let Ok(vec_data) = cirru_edn::from_edn::<Vec<Edn>>(edn_data) {
        for item in vec_data {
          if let Ok(doc) = cirru_edn::from_edn::<ApiDoc>(item) {
            api_docs.insert(doc.name.clone(), doc);
          }
        }
      }
    }
  }

  Ok(api_docs)
}

fn load_guidebook_docs() -> Result<HashMap<String, GuideDoc>, String> {
  let docs_dir = get_guidebook_dir()?;
  let mut guide_docs = HashMap::new();

  fn visit_dir(dir: &Path, base_dir: &Path, docs: &mut HashMap<String, GuideDoc>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {e}"))? {
      let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
      let path = entry.path();

      if path.is_dir() {
        visit_dir(&path, base_dir, docs)?;
      } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
        let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read file {path:?}: {e}"))?;
        let relative_path = path.strip_prefix(base_dir).map_err(|_| "Unable to get relative path")?;
        let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();

        docs.insert(
          filename.clone(),
          GuideDoc {
            filename,
            path: relative_path.to_string_lossy().to_string(),
            content,
          },
        );
      }
    }
    Ok(())
  }

  visit_dir(&docs_dir, &docs_dir, &mut guide_docs)?;
  Ok(guide_docs)
}

fn handle_api(query_type: &str, query_value: Option<&str>) -> Result<(), String> {
  let api_docs = load_api_docs()?;

  let results: Vec<&ApiDoc> = match query_type {
    "all" => api_docs.values().collect(),
    "tag" => {
      let tag = query_value.ok_or("query value is required for tag queries")?;
      api_docs.values().filter(|doc| doc.tags.iter().any(|t| t.contains(tag))).collect()
    }
    "keyword" => {
      let keyword = query_value.ok_or("query value is required for keyword queries")?;
      api_docs
        .values()
        .filter(|doc| doc.name.contains(keyword) || doc.desc.contains(keyword) || doc.tags.iter().any(|t| t.contains(keyword)))
        .collect()
    }
    _ => {
      return Err(format!("Invalid query_type '{query_type}'. Valid types are: 'all', 'tag', 'keyword'"));
    }
  };

  if results.is_empty() {
    println!("{}", "No matching API documentation found.".yellow());
    return Ok(());
  }

  println!("{} {} results\n", "Found".bold(), results.len());

  for doc in results {
    println!("{}", doc.name.cyan().bold());
    println!("  {}", doc.desc);
    if !doc.tags.is_empty() {
      let tags: Vec<&String> = doc.tags.iter().collect();
      println!("  {}: {}", "Tags".dimmed(), tags.iter().map(|t| t.yellow().to_string()).collect::<Vec<_>>().join(", "));
    }
    if !doc.snippets.is_empty() {
      println!("  {}:", "Examples".dimmed());
      for snippet in &doc.snippets {
        let snippet_str = cirru_edn::format(snippet, true).unwrap_or_else(|_| "(failed to format)".to_string());
        println!("    {}", snippet_str.green());
      }
    }
    println!();
  }

  Ok(())
}

fn handle_ref(query_type: &str, query_value: Option<&str>) -> Result<(), String> {
  let guide_docs = load_guidebook_docs()?;

  let results: Vec<&GuideDoc> = match query_type {
    "all" => guide_docs.values().collect(),
    "filename" => {
      let filename = query_value.ok_or("query value is required for filename queries")?;
      guide_docs
        .values()
        .filter(|doc| doc.filename.contains(filename) || doc.path.contains(filename))
        .collect()
    }
    "keyword" => {
      let keyword = query_value.ok_or("query value is required for keyword queries")?;
      guide_docs
        .values()
        .filter(|doc| doc.filename.contains(keyword) || doc.path.contains(keyword) || doc.content.contains(keyword))
        .collect()
    }
    _ => {
      return Err(format!(
        "Invalid query_type '{query_type}'. Valid types are: 'all', 'filename', 'keyword'"
      ));
    }
  };

  if results.is_empty() {
    println!("{}", "No matching guidebook documentation found.".yellow());
    return Ok(());
  }

  println!("{} {} results\n", "Found".bold(), results.len());

  for doc in results {
    println!("{} ({})", doc.filename.cyan().bold(), doc.path.dimmed());
    println!("{}", "-".repeat(60).dimmed());

    // Print content, potentially truncated for "all" query
    if query_type == "all" && doc.content.len() > 500 {
      println!("{}...\n", &doc.content[..500]);
    } else {
      println!("{}\n", doc.content);
    }
  }

  Ok(())
}

fn handle_list_api() -> Result<(), String> {
  let api_docs = load_api_docs()?;

  println!("{}", "Available API Documentation:".bold());

  let mut names: Vec<&String> = api_docs.keys().collect();
  names.sort();

  for name in &names {
    println!("  {}", name.cyan());
  }

  println!("\n{} {} topics", "Total:".dimmed(), names.len());
  println!("{}", "Use 'cr docs api <keyword>' to search for specific APIs".dimmed());

  Ok(())
}

fn handle_list_guide() -> Result<(), String> {
  let guide_docs = load_guidebook_docs()?;

  println!("{}", "Available Guidebook Documentation:".bold());

  let mut docs: Vec<&GuideDoc> = guide_docs.values().collect();
  docs.sort_by_key(|d| &d.path);

  for doc in &docs {
    let preview = if doc.content.len() > 100 {
      format!("{}...", doc.content.lines().next().unwrap_or(""))
    } else {
      doc.content.lines().next().unwrap_or("").to_string()
    };

    println!("  {} - {}", doc.filename.cyan(), preview.dimmed());
  }

  println!("\n{} {} topics", "Total:".dimmed(), docs.len());
  println!("{}", "Use 'cr docs ref <keyword>' to search guidebook content".dimmed());

  Ok(())
}
