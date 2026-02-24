//! Docs subcommand handlers
//!
//! Handles: cr docs search, read, list

use calcit::cli_args::{DocsCommand, DocsSubcommand};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct GuideDoc {
  filename: String,
  path: String,
  content: String,
}

pub fn handle_docs_command(cmd: &DocsCommand) -> Result<(), String> {
  match &cmd.subcommand {
    DocsSubcommand::Search(opts) => handle_search(&opts.keyword, opts.context, opts.filename.as_deref()),
    DocsSubcommand::Read(opts) => handle_read(&opts.filename, opts.start, opts.lines),
    DocsSubcommand::List(_) => handle_list(),
    DocsSubcommand::CheckMd(opts) => handle_check_md(&opts.file, &opts.entry),
  }
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

fn handle_search(keyword: &str, context_lines: usize, filename_filter: Option<&str>) -> Result<(), String> {
  let guide_docs = load_guidebook_docs()?;

  let mut found_any = false;

  for doc in guide_docs.values() {
    // Skip SUMMARY files
    if doc.filename.to_uppercase().contains("SUMMARY") {
      continue;
    }

    // Apply filename filter if provided
    if let Some(filter) = filename_filter {
      if !doc.filename.contains(filter) {
        continue;
      }
    }

    let lines: Vec<&str> = doc.content.lines().collect();
    let mut matching_ranges: Vec<(usize, usize)> = Vec::new();

    // Find all matching lines
    for (line_num, line) in lines.iter().enumerate() {
      if line.contains(keyword) {
        let start = line_num.saturating_sub(context_lines);
        let end = (line_num + context_lines + 1).min(lines.len());
        matching_ranges.push((start, end));
      }
    }

    if matching_ranges.is_empty() {
      continue;
    }

    found_any = true;

    // Merge overlapping ranges
    matching_ranges.sort_by_key(|r| r.0);
    let mut merged_ranges: Vec<(usize, usize)> = Vec::new();
    for (start, end) in matching_ranges {
      if let Some(last) = merged_ranges.last_mut() {
        if start <= last.1 {
          last.1 = last.1.max(end);
          continue;
        }
      }
      merged_ranges.push((start, end));
    }

    // Display matches
    println!("{} ({})", doc.filename.cyan().bold(), doc.path.dimmed());
    println!("{}", "-".repeat(60).dimmed());

    for (start, end) in merged_ranges {
      for (idx, line) in lines[start..end].iter().enumerate() {
        let line_num = start + idx + 1;
        if line.contains(keyword) {
          println!("{} {}", format!("{line_num:4}:").yellow(), line);
        } else {
          println!("{} {}", format!("{line_num:4}:").dimmed(), line.dimmed());
        }
      }
      println!();
    }
  }

  if !found_any {
    println!("{}", "No matching content found.".yellow());
  } else {
    println!(
      "{}",
      "Tip: Use -c <num> to show more context lines (e.g., 'cr docs search <keyword> -c 20')".dimmed()
    );
    if filename_filter.is_none() {
      println!(
        "{}",
        "     Use -f <filename> to filter by file (e.g., 'cr docs search <keyword> -f syntax.md')".dimmed()
      );
    }
  }

  Ok(())
}

fn handle_read(filename: &str, start: usize, lines_to_read: usize) -> Result<(), String> {
  let guide_docs = load_guidebook_docs()?;

  // Try to find the document by exact filename match or contains
  let doc = guide_docs
    .values()
    .find(|d| d.filename == filename || d.filename.contains(filename))
    .ok_or_else(|| format!("Document '{filename}' not found. Use 'cr docs list' to see available documents."))?;

  let all_lines: Vec<&str> = doc.content.lines().collect();
  let total_lines = all_lines.len();
  let end = (start + lines_to_read).min(total_lines);

  println!("{} ({})", doc.filename.cyan().bold(), doc.path.dimmed());
  println!("{}", "=".repeat(60).dimmed());

  // Display lines with line numbers
  for (idx, line) in all_lines[start..end].iter().enumerate() {
    let line_num = start + idx + 1;
    println!("{} {}", format!("{line_num:4}:").dimmed(), line);
  }

  // Show tips
  println!();
  println!(
    "{}",
    format!("Lines {}-{} of {} (total {} lines)", start + 1, end, total_lines, total_lines).dimmed()
  );

  if end < total_lines {
    let remaining = total_lines - end;
    println!(
      "{}",
      format!("More content available ({remaining} lines remaining). Use -s {end} -n {lines_to_read} to continue reading.").yellow()
    );
  } else {
    println!("{}", "End of document.".green());
  }

  println!(
    "{}",
    "Tip: Use -s <start> -n <lines> to read specific range (e.g., 'cr docs read file.md -s 20 -n 30')".dimmed()
  );

  Ok(())
}

fn handle_list() -> Result<(), String> {
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
  println!("{}", "Use 'cr docs read <filename>' to read a document".dimmed());
  println!("{}", "    'cr docs search <keyword>' to search content".dimmed());

  Ok(())
}

/// Check mode for a cirru code block in markdown.
///
/// - `cirru`          → Run: parse + preprocess + eval
/// - `cirru.no-run`   → NoRun: parse + preprocess (type-check), skip eval
/// - `cirru.no-check` → NoCheck: parse only (syntax validation)
#[derive(Debug, Clone, Copy, PartialEq)]
enum CirruCheckMode {
  Run,
  NoRun,
  NoCheck,
}

impl std::fmt::Display for CirruCheckMode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      CirruCheckMode::Run => write!(f, "run"),
      CirruCheckMode::NoRun => write!(f, "no-run"),
      CirruCheckMode::NoCheck => write!(f, "no-check"),
    }
  }
}

/// Extract cirru code blocks from markdown content.
/// Recognizes:  ```cirru  ```cirru.no-run  ```cirru.no-check
/// Returns (line_number, check_mode, code_content) triples.
fn extract_cirru_blocks(content: &str) -> Vec<(usize, CirruCheckMode, String)> {
  let mut blocks = Vec::new();
  let mut in_block = false;
  let mut in_non_cirru_block = false;
  let mut block_start_line = 0;
  let mut block_mode = CirruCheckMode::Run;
  let mut block_lines: Vec<&str> = Vec::new();

  for (idx, line) in content.lines().enumerate() {
    let trimmed = line.trim();
    if !in_block && !in_non_cirru_block && trimmed.starts_with("```") {
      if trimmed == "```cirru" {
        in_block = true;
        block_start_line = idx + 1; // 1-based
        block_mode = CirruCheckMode::Run;
        block_lines.clear();
      } else if trimmed == "```cirru.no-run" {
        in_block = true;
        block_start_line = idx + 1;
        block_mode = CirruCheckMode::NoRun;
        block_lines.clear();
      } else if trimmed == "```cirru.no-check" {
        in_block = true;
        block_start_line = idx + 1;
        block_mode = CirruCheckMode::NoCheck;
        block_lines.clear();
      } else {
        // other fenced block (```json, ```bash, etc.) — skip entirely
        in_non_cirru_block = true;
      }
    } else if (in_block || in_non_cirru_block) && trimmed == "```" {
      if in_block && !block_lines.is_empty() {
        blocks.push((block_start_line, block_mode, block_lines.join("\n")));
      }
      in_block = false;
      in_non_cirru_block = false;
    } else if in_block {
      block_lines.push(line);
    }
  }

  blocks
}

fn handle_check_md(file_path: &str, entry: &str) -> Result<(), String> {
  let content = fs::read_to_string(file_path).map_err(|e| format!("Failed to read file '{file_path}': {e}"))?;

  let blocks = extract_cirru_blocks(&content);

  if blocks.is_empty() {
    println!("{}", "No cirru code blocks found in the file.".yellow());
    return Ok(());
  }

  // Resolve the cr binary path (use current executable)
  let cr_bin = std::env::current_exe().map_err(|e| format!("Failed to get current executable path: {e}"))?;

  if !Path::new(entry).exists() {
    return Err(format!(
      "Entry file '{entry}' not found. Use -d to specify a valid entry .cirru file."
    ));
  }

  println!("{} {} (entry: {})", "Checking".bold(), file_path.cyan(), entry.dimmed());
  println!("{}", "-".repeat(60).dimmed());

  let mut passed = 0;
  let mut failed = 0;
  let total = blocks.len();

  for (line_num, mode, code) in &blocks {
    // Show a brief preview of the code block
    let preview: String = code.lines().next().unwrap_or("").chars().take(60).collect();
    let preview_suffix = if code.lines().count() > 1 || preview.len() >= 60 {
      "..."
    } else {
      ""
    };

    let mode_label = match mode {
      CirruCheckMode::Run => "",
      CirruCheckMode::NoRun => "[no-run] ",
      CirruCheckMode::NoCheck => "[no-check] ",
    };

    let output = match mode {
      CirruCheckMode::Run => {
        // parse + preprocess + eval
        Command::new(&cr_bin)
          .arg(entry)
          .arg("eval")
          .arg("--")
          .arg(code)
          .output()
          .map_err(|e| format!("Failed to run eval: {e}"))?
      }
      CirruCheckMode::NoRun => {
        // parse + preprocess (--check-only), skip eval
        Command::new(&cr_bin)
          .arg("--check-only")
          .arg(entry)
          .arg("eval")
          .arg("--")
          .arg(code)
          .output()
          .map_err(|e| format!("Failed to run check-only: {e}"))?
      }
      CirruCheckMode::NoCheck => {
        // parse only via `cr cirru parse` (multi-line indentation parser)
        Command::new(&cr_bin)
          .arg("cirru")
          .arg("parse")
          .arg(code)
          .output()
          .map_err(|e| format!("Failed to run cirru parse: {e}"))?
      }
    };

    if output.status.success() {
      passed += 1;
      println!(
        "  {} L{}: {}{}{}",
        "✓".green(),
        format!("{line_num}").dimmed(),
        mode_label.dimmed(),
        preview.dimmed(),
        preview_suffix.dimmed()
      );
    } else {
      failed += 1;
      let stderr = String::from_utf8_lossy(&output.stderr);
      // Extract the error line (usually the line with "Error:" or "Failure:")
      let error_msg: String = stderr
        .lines()
        .find(|l| l.contains("Error:") || l.contains("Failure:"))
        .or_else(|| stderr.lines().rev().find(|l| !l.trim().is_empty()))
        .unwrap_or("(unknown error)")
        .to_string();
      println!(
        "  {} L{}: {}{}{}",
        "✗".red(),
        format!("{line_num}").yellow(),
        mode_label,
        preview,
        preview_suffix
      );
      println!("    {}", error_msg.red());
    }
  }

  println!("{}", "-".repeat(60).dimmed());
  let summary = format!("Results: {total} blocks, {passed} passed, {failed} failed");
  if failed > 0 {
    println!("{}", summary.red().bold());
    Err(format!("{failed} code block(s) failed"))
  } else {
    println!("{}", summary.green().bold());
    Ok(())
  }
}
