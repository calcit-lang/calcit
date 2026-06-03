//! Docs subcommand handlers
//!
//! Handles: cr docs scopes, search, list, sections, read, read-lines

use calcit::cli_args::{DocsCommand, DocsSubcommand};
use colored::Colorize;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Once;
use std::time::{Duration, SystemTime};

use calcit::ProgramEntries;
use calcit::calcit::LocatedWarning;
use calcit::call_stack::CallStackList;
use calcit::program;
use calcit::runner;
use calcit::snapshot;
use calcit::util;

use super::libs::handle_libs_command;
use super::markdown_read::{RenderMarkdownOptions, render_markdown_sections};
use super::tips::command_guidance_enabled;

const VALID_DOC_CATEGORIES: &[&str] = &[
  "run",
  "features",
  "installation",
  "data",
  "intro",
  "syntax",
  "tools",
  "ecosystem",
  "reference",
  "docs",
];

#[derive(Debug, Clone)]
pub struct GuideDoc {
  filename: String,
  path: String,
  content: String,
  scope: GuideDocScope,
  frontmatter: GuideDocFrontmatter,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct GuideDocFrontmatter {
  title: Option<String>,
  scope: Option<String>,
  kind: Option<String>,
  category: Option<String>,
  aliases: Vec<String>,
  entry_for: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GuideDocScope {
  Core,
  Module(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchResult {
  doc_index: usize,
  merged_ranges: Vec<(usize, usize)>,
  score: usize,
}

impl GuideDoc {
  fn display_title(&self) -> String {
    let base_title = self.frontmatter.title.as_deref().unwrap_or(&self.filename);
    match &self.scope {
      GuideDocScope::Core => base_title.to_string(),
      GuideDocScope::Module(module) => format!("{base_title} [module:{module}]"),
    }
  }
}

fn match_score(value: &str, query_lower: &str, exact_score: usize, contains_score: usize) -> usize {
  let value_lower = value.to_lowercase();
  if value_lower == query_lower {
    exact_score
  } else if value_lower.contains(query_lower) {
    contains_score
  } else {
    0
  }
}

fn accumulate_match_score(total: &mut usize, matched: &mut bool, value: &str, query_lower: &str, exact: usize, contains: usize) {
  let score = match_score(value, query_lower, exact, contains);
  if score > 0 {
    *total += score;
    *matched = true;
  }
}

fn parse_guide_doc(filename: String, path: String, raw_content: &str, scope: GuideDocScope) -> Result<GuideDoc, String> {
  let (frontmatter, content) = parse_doc_frontmatter(raw_content);
  validate_doc_frontmatter(&path, &frontmatter)?;
  Ok(GuideDoc {
    filename,
    path,
    content,
    scope,
    frontmatter,
  })
}

struct ReadRenderOptions<'a> {
  display_title: &'a str,
  display_path: &'a str,
  command_hint: &'a str,
  no_match_error: &'a str,
  content: &'a str,
  heading_queries: &'a [String],
  include_subheadings: bool,
  full: bool,
  with_lines: bool,
}

pub fn handle_docs_command(cmd: &DocsCommand) -> Result<(), String> {
  match &cmd.subcommand {
    DocsSubcommand::Scopes(_) => handle_scopes(),
    DocsSubcommand::RemoteLibs(opts) => handle_libs_command(&calcit::cli_args::LibsCommand {
      subcommand: opts.subcommand.clone(),
    }),
    DocsSubcommand::Search(opts) => handle_search(&opts.keyword, opts.context, opts.filename.as_deref(), opts.module.as_deref()),
    DocsSubcommand::List(opts) => handle_list(opts.module.as_deref()),
    DocsSubcommand::Sections(opts) => handle_sections(&opts.filename, opts.module.as_deref(), opts.with_lines),
    DocsSubcommand::Read(opts) => handle_read(
      &opts.filename,
      &opts.headings,
      !opts.no_subheadings,
      opts.full,
      opts.with_lines,
      opts.module.as_deref(),
    ),
    DocsSubcommand::Agents(opts) => handle_agents(&opts.headings, !opts.no_subheadings, opts.full, opts.with_lines, opts.refresh),
    DocsSubcommand::ReadLines(opts) => handle_read_lines(&opts.filename, opts.start, opts.lines, opts.module.as_deref()),
    DocsSubcommand::CheckMd(opts) => handle_check_md(&opts.file, &opts.entry, &opts.dep),
  }
}

const AGENTS_DOC_URL: &str = "https://repo.calcit-lang.org/calcit/docs/CalcitAgent.md";

fn get_agents_cache_path() -> Result<PathBuf, String> {
  let home_dir = std::env::var("HOME").map_err(|_| "Unable to get HOME environment variable".to_string())?;
  Ok(Path::new(&home_dir).join(".config/calcit/Agents.md"))
}

fn needs_agents_refresh(cache_path: &Path) -> bool {
  if !cache_path.exists() {
    return true;
  }

  let metadata = match fs::metadata(cache_path) {
    Ok(m) => m,
    Err(_) => return true,
  };

  let modified = match metadata.modified() {
    Ok(m) => m,
    Err(_) => return true,
  };

  let now = SystemTime::now();
  match now.duration_since(modified) {
    Ok(age) => age > Duration::from_secs(60 * 60),
    Err(_) => true,
  }
}

fn download_agents_doc() -> Result<String, String> {
  let response = ureq::get(AGENTS_DOC_URL)
    .call()
    .map_err(|e| format!("Failed to download Agents.md: {e}"))?;

  response
    .into_body()
    .read_to_string()
    .map_err(|e| format!("Failed to read Agents.md response: {e}"))
}

fn ensure_agents_cache(force_refresh: bool) -> Result<(PathBuf, bool), String> {
  let cache_path = get_agents_cache_path()?;
  let should_refresh = force_refresh || needs_agents_refresh(&cache_path);

  if should_refresh {
    let content = download_agents_doc()?;
    if let Some(parent) = cache_path.parent() {
      fs::create_dir_all(parent).map_err(|e| format!("Failed to create cache directory {parent:?}: {e}"))?;
    }
    fs::write(&cache_path, content).map_err(|e| format!("Failed to write Agents cache {cache_path:?}: {e}"))?;
  }

  Ok((cache_path, should_refresh))
}

fn handle_read_content(options: ReadRenderOptions<'_>) -> Result<(), String> {
  let command_prefix = format!("cr docs {}", options.command_hint);

  println!("{} ({})", options.display_title.cyan().bold(), options.display_path.dimmed());
  println!("{}", "=".repeat(60).dimmed());

  render_markdown_sections(
    options.content,
    options.heading_queries,
    RenderMarkdownOptions {
      include_subheadings: options.include_subheadings,
      full: options.full,
      with_lines: options.with_lines,
      command_prefix: &command_prefix,
      with_file_option: false,
      no_headings_message: "No Markdown headings found in this document.",
      print_full_when_no_headings: false,
      no_match_error: options.no_match_error,
    },
  )
}

fn score_doc_query(doc: &GuideDoc, query_lower: &str) -> usize {
  let mut score: usize = 0;
  score += match_score(&doc.filename, query_lower, 320, 180);
  score += match_score(&doc.path, query_lower, 280, 140);

  if let Some(title) = &doc.frontmatter.title {
    score += match_score(title, query_lower, 260, 150);
  }

  for alias in &doc.frontmatter.aliases {
    score += match_score(alias, query_lower, 220, 120);
  }

  for entry in &doc.frontmatter.entry_for {
    score += match_score(entry, query_lower, 180, 96);
  }

  score.saturating_add_signed(score_doc_shape(doc))
}

fn find_doc_by_query<'a>(guide_docs: &'a [GuideDoc], query: &str) -> Result<&'a GuideDoc, String> {
  let query_lower = query.to_lowercase();
  guide_docs
    .iter()
    .filter_map(|doc| {
      let score = score_doc_query(doc, &query_lower);
      (score > 0).then_some((doc, score))
    })
    .max_by(|(left_doc, left_score), (right_doc, right_score)| {
      left_score.cmp(right_score).then_with(|| right_doc.path.cmp(&left_doc.path))
    })
    .map(|(doc, _)| doc)
    .ok_or_else(|| {
      format!("Document '{query}' not found. Use 'cr docs list' or 'cr docs search {query}' to locate matching documents.")
    })
}

fn get_guidebook_dir() -> Result<PathBuf, String> {
  let home_dir = std::env::var("HOME").map_err(|_| "Unable to get HOME environment variable")?;
  let docs_dir = Path::new(&home_dir).join(".config/calcit/docs");

  if !docs_dir.exists() {
    let calcit_repo_dir = Path::new(&home_dir).join(".config/calcit/calcit");
    return Err(format!(
      "Guidebook documentation directory not found: {docs_dir:?}\n\nDownload the Calcit docs repo with git, then create a symlink for the docs directory:\nmkdir -p ~/.config/calcit\ngit clone https://github.com/calcit-lang/calcit.git {}\nln -s {}/docs {}",
      calcit_repo_dir.display(),
      calcit_repo_dir.display(),
      docs_dir.display()
    ));
  }

  Ok(docs_dir)
}

fn trim_frontmatter_value(raw: &str) -> String {
  let value = raw.trim();
  if value.len() >= 2 {
    let first = value.chars().next().unwrap_or_default();
    let last = value.chars().last().unwrap_or_default();
    if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
      return value[1..value.len() - 1].to_string();
    }
  }
  value.to_string()
}

fn validate_doc_frontmatter(path: &str, frontmatter: &GuideDocFrontmatter) -> Result<(), String> {
  if let Some(category) = frontmatter.category.as_deref() {
    if !VALID_DOC_CATEGORIES.contains(&category) {
      return Err(format!(
        "Invalid frontmatter category '{category}' in {path}. Use one of: {}. See docs/docs-indexing.md.",
        VALID_DOC_CATEGORIES.join(", ")
      ));
    }
  }

  Ok(())
}

fn parse_doc_frontmatter(raw: &str) -> (GuideDocFrontmatter, String) {
  if !raw.starts_with("---\n") {
    return (GuideDocFrontmatter::default(), raw.to_string());
  }

  let mut frontmatter = GuideDocFrontmatter::default();
  let mut body_lines = Vec::new();
  let mut in_frontmatter = true;
  let mut saw_closing = false;
  let mut active_list: Option<&str> = None;

  for (index, line) in raw.lines().enumerate() {
    if index == 0 {
      continue;
    }

    if in_frontmatter {
      if line.trim() == "---" {
        in_frontmatter = false;
        saw_closing = true;
        active_list = None;
        continue;
      }

      let trimmed = line.trim();
      if let Some(item) = trimmed.strip_prefix("- ") {
        let value = trim_frontmatter_value(item);
        match active_list {
          Some("aliases") => frontmatter.aliases.push(value),
          Some("entry_for") => frontmatter.entry_for.push(value),
          _ => {}
        }
        continue;
      }

      if let Some((key, value)) = trimmed.split_once(':') {
        let key = key.trim();
        let value = value.trim();
        active_list = None;
        match key {
          "title" => frontmatter.title = Some(trim_frontmatter_value(value)),
          "scope" => frontmatter.scope = Some(trim_frontmatter_value(value)),
          "kind" => frontmatter.kind = Some(trim_frontmatter_value(value)),
          "category" => frontmatter.category = Some(trim_frontmatter_value(value)),
          "aliases" if value.is_empty() => active_list = Some("aliases"),
          "entry_for" if value.is_empty() => active_list = Some("entry_for"),
          "aliases" => frontmatter.aliases.push(trim_frontmatter_value(value)),
          "entry_for" => frontmatter.entry_for.push(trim_frontmatter_value(value)),
          _ => {}
        }
        continue;
      }

      active_list = None;
    } else {
      body_lines.push(line);
    }
  }

  if !saw_closing {
    return (GuideDocFrontmatter::default(), raw.to_string());
  }

  let body = body_lines.join("\n").trim_start_matches('\n').to_string();
  (frontmatter, body)
}

fn visit_markdown_dir(dir: &Path, base_dir: &Path, docs: &mut Vec<GuideDoc>, scope: &GuideDocScope) -> Result<(), String> {
  for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {e}"))? {
    let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
    let path = entry.path();

    if path.is_dir() {
      if path.file_name().and_then(|s| s.to_str()).is_some_and(|name| name.starts_with('.')) {
        continue;
      }
      visit_markdown_dir(&path, base_dir, docs, scope)?;
    } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
      let raw_content = fs::read_to_string(&path).map_err(|e| format!("Failed to read file {path:?}: {e}"))?;
      let relative_path = path.strip_prefix(base_dir).map_err(|_| "Unable to get relative path")?;
      let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
      let relative_path_str = relative_path.to_string_lossy().to_string();
      docs.push(parse_guide_doc(filename, relative_path_str, &raw_content, scope.clone())?);
    }
  }
  Ok(())
}

fn load_guidebook_docs() -> Result<Vec<GuideDoc>, String> {
  let docs_dir = get_guidebook_dir()?;
  let mut guide_docs = Vec::new();
  visit_markdown_dir(&docs_dir, &docs_dir, &mut guide_docs, &GuideDocScope::Core)?;
  Ok(guide_docs)
}

fn load_module_docs_from_dir(modules_dir: &Path, module_filter: Option<&str>) -> Result<Vec<GuideDoc>, String> {
  let mut docs = Vec::new();
  let mut seen_modules = HashSet::new();

  for entry in fs::read_dir(modules_dir).map_err(|e| format!("Failed to read modules directory {modules_dir:?}: {e}"))? {
    let entry = entry.map_err(|e| format!("Failed to read modules directory entry: {e}"))?;
    let path = entry.path();
    if !path.is_dir() {
      continue;
    }

    let module_name = match path.file_name().and_then(|s| s.to_str()) {
      Some(name) if !name.starts_with('.') => name.to_string(),
      _ => continue,
    };

    if let Some(filter) = module_filter {
      if module_name != filter {
        continue;
      }
    }

    seen_modules.insert(module_name.clone());
    let scope = GuideDocScope::Module(module_name.clone());

    let agents_path = path.join("Agents.md");
    if agents_path.exists() {
      let raw_content = fs::read_to_string(&agents_path).map_err(|e| format!("Failed to read file {agents_path:?}: {e}"))?;
      let agents_doc_path = format!("{module_name}/Agents.md");
      docs.push(parse_guide_doc(
        "Agents.md".to_string(),
        agents_doc_path,
        &raw_content,
        scope.clone(),
      )?);
    }

    let docs_path = path.join("docs");
    if docs_path.exists() {
      visit_markdown_dir(&docs_path, &path, &mut docs, &scope)?;
    }
  }

  if let Some(filter) = module_filter {
    if !seen_modules.contains(filter) {
      return Err(format!(
        "Module '{filter}' not found under {modules_dir:?}. Use 'cr docs list --module <module>' or inspect ~/.config/calcit/modules/."
      ));
    }
  }

  Ok(docs)
}

fn load_module_docs(module_filter: Option<&str>) -> Result<Vec<GuideDoc>, String> {
  let modules_dir = module_folder()?;
  load_module_docs_from_dir(&modules_dir, module_filter)
}

fn collect_docs_for_query(module_filter: Option<&str>) -> Result<Vec<GuideDoc>, String> {
  let mut docs = if module_filter.is_some() {
    load_module_docs(module_filter)?
  } else {
    load_guidebook_docs()?
  };
  docs.sort_by(|a, b| a.path.cmp(&b.path));
  Ok(docs)
}

fn list_doc_scopes() -> Result<Vec<String>, String> {
  let modules_dir = module_folder()?;
  let mut scopes = vec!["calcit".to_string()];

  for entry in fs::read_dir(&modules_dir).map_err(|e| format!("Failed to read modules directory {modules_dir:?}: {e}"))? {
    let entry = entry.map_err(|e| format!("Failed to read modules directory entry: {e}"))?;
    let path = entry.path();
    if !path.is_dir() {
      continue;
    }

    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
      if !name.starts_with('.') {
        scopes.push(name.to_string());
      }
    }
  }

  scopes.sort();
  scopes.dedup();
  Ok(scopes)
}

fn score_metadata_hit(doc: &GuideDoc, keyword_lower: &str) -> (usize, bool) {
  let mut score = 0;
  let mut matched = false;

  if let Some(title) = &doc.frontmatter.title {
    accumulate_match_score(&mut score, &mut matched, title, keyword_lower, 240, 160);
  }

  for alias in &doc.frontmatter.aliases {
    accumulate_match_score(&mut score, &mut matched, alias, keyword_lower, 220, 120);
  }

  for entry in &doc.frontmatter.entry_for {
    accumulate_match_score(&mut score, &mut matched, entry, keyword_lower, 180, 90);
  }

  (score, matched)
}

fn score_doc_shape(doc: &GuideDoc) -> isize {
  let kind_score = match doc.frontmatter.kind.as_deref() {
    Some("reference") => 18,
    Some("guide") => 14,
    Some("hub") => 8,
    Some("agent") => 4,
    Some("spec") => -6,
    Some(_) | None => 0,
  };

  let category_score = match doc.frontmatter.category.as_deref() {
    Some("run") | Some("features") | Some("installation") | Some("data") | Some("intro") => 4,
    Some("docs") => -2,
    Some(_) | None => 0,
  };

  let scope_score = match (&doc.scope, doc.frontmatter.scope.as_deref()) {
    (GuideDocScope::Module(_), Some("module")) => 4,
    (GuideDocScope::Core, Some("core")) => 2,
    _ => 0,
  };

  kind_score + category_score + scope_score
}

fn score_search_hit(doc: &GuideDoc, lines: &[&str], keyword_lower: &str, matching_lines: &[usize]) -> usize {
  let (mut score, _) = score_metadata_hit(doc, keyword_lower);
  score = score.saturating_add_signed(score_doc_shape(doc));
  score += match_score(&doc.filename, keyword_lower, 140, 80);
  score += match_score(&doc.path, keyword_lower, 120, 36);

  for line_index in matching_lines {
    let line = lines[*line_index];
    let lower = line.to_lowercase();
    let trimmed = line.trim_start();
    let trimmed_lower = lower.trim_start();

    if trimmed.starts_with('#') {
      score += 48;
    } else {
      score += 10;
    }

    if trimmed_lower == keyword_lower {
      score += 36;
    }

    if trimmed_lower.starts_with(keyword_lower) {
      score += 18;
    }

    if lower.contains(&format!("`{keyword_lower}`")) {
      score += 14;
    }

    if *line_index < 40 {
      score += 6;
    }
  }

  score
}

fn merge_ranges(mut matching_ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
  matching_ranges.sort_by_key(|range| range.0);
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

  merged_ranges
}

fn collect_search_results(
  guide_docs: &[GuideDoc],
  keyword_lower: &str,
  context_lines: usize,
  filename_filter: Option<&str>,
) -> Vec<SearchResult> {
  let mut results = Vec::new();

  for (doc_index, doc) in guide_docs.iter().enumerate() {
    if doc.filename.to_uppercase().contains("SUMMARY") {
      continue;
    }

    if let Some(filter) = filename_filter {
      if !doc.filename.contains(filter) && !doc.path.contains(filter) {
        continue;
      }
    }

    let lines: Vec<&str> = doc.content.lines().collect();
    let mut matching_ranges: Vec<(usize, usize)> = Vec::new();
    let mut matching_lines: Vec<usize> = Vec::new();
    let (_, metadata_matched) = score_metadata_hit(doc, keyword_lower);

    for (line_num, line) in lines.iter().enumerate() {
      if line.to_lowercase().contains(keyword_lower) {
        let start = line_num.saturating_sub(context_lines);
        let end = (line_num + context_lines + 1).min(lines.len());
        matching_ranges.push((start, end));
        matching_lines.push(line_num);
      }
    }

    if matching_ranges.is_empty() && !metadata_matched {
      continue;
    }

    if matching_ranges.is_empty() {
      let preview_end = lines.len().min(context_lines.saturating_mul(2).max(6));
      if preview_end > 0 {
        matching_ranges.push((0, preview_end));
      }
    }

    results.push(SearchResult {
      doc_index,
      merged_ranges: merge_ranges(matching_ranges),
      score: score_search_hit(doc, &lines, keyword_lower, &matching_lines),
    });
  }

  results.sort_by(|a, b| {
    b.score
      .cmp(&a.score)
      .then_with(|| guide_docs[a.doc_index].path.cmp(&guide_docs[b.doc_index].path))
  });

  results
}

fn handle_scopes() -> Result<(), String> {
  let scopes = list_doc_scopes()?;

  println!("{}", "Available Documentation Scopes:".bold());
  println!("{}", "calcit".cyan().bold());

  for scope in scopes.iter().filter(|scope| scope.as_str() != "calcit") {
    println!("  {}", scope.cyan());
  }

  println!("\n{} {} scopes", "Total:".dimmed(), scopes.len());
  if command_guidance_enabled() {
    println!("{}", "Use 'cr docs list' to list guidebook files in calcit.".dimmed());
    println!(
      "{}",
      "    Use 'cr docs list --module <module>' to list files in an installed module.".dimmed()
    );
  }

  Ok(())
}

fn handle_search(
  keyword: &str,
  context_lines: usize,
  filename_filter: Option<&str>,
  module_filter: Option<&str>,
) -> Result<(), String> {
  let guide_docs = collect_docs_for_query(module_filter)?;
  let keyword_lower = keyword.to_lowercase();

  let results = collect_search_results(&guide_docs, &keyword_lower, context_lines, filename_filter);

  for result in &results {
    let doc = &guide_docs[result.doc_index];
    let lines: Vec<&str> = doc.content.lines().collect();

    println!("{} ({})", doc.display_title().cyan().bold(), doc.path.dimmed());
    println!("{}", "-".repeat(60).dimmed());

    for (start, end) in &result.merged_ranges {
      for (idx, line) in lines[*start..*end].iter().enumerate() {
        let line_num = *start + idx + 1;
        if line.to_lowercase().contains(&keyword_lower) {
          println!("{} {}", format!("{line_num:4}:").yellow(), line);
        } else {
          println!("{} {}", format!("{line_num:4}:").dimmed(), line.dimmed());
        }
      }
      println!();
    }
  }

  if results.is_empty() {
    println!("{}", "No matching content found.".yellow());
  } else if command_guidance_enabled() {
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
    if module_filter.is_none() {
      println!(
        "{}",
        "     Use --module <name> to search one installed module instead of calcit docs.".dimmed()
      );
    }
  }

  Ok(())
}

fn handle_sections(filename: &str, module_filter: Option<&str>, with_lines: bool) -> Result<(), String> {
  let guide_docs = collect_docs_for_query(module_filter)?;
  let doc = find_doc_by_query(&guide_docs, filename)?;

  handle_read_content(ReadRenderOptions {
    display_title: &doc.display_title(),
    display_path: &doc.path,
    command_hint: "sections",
    no_match_error: "No sections found in document.",
    content: &doc.content,
    heading_queries: &[],
    include_subheadings: true,
    full: false,
    with_lines,
  })
}

fn handle_agents(
  heading_queries: &[String],
  include_subheadings: bool,
  full: bool,
  with_lines: bool,
  force_refresh: bool,
) -> Result<(), String> {
  let (cache_path, refreshed) = ensure_agents_cache(force_refresh)?;
  if refreshed {
    if force_refresh {
      println!("{}", format!("Force refreshed Agents doc cache from: {AGENTS_DOC_URL}").dimmed());
    } else {
      println!("{}", format!("Refreshed Agents doc cache from: {AGENTS_DOC_URL}").dimmed());
    }
  }
  let content = fs::read_to_string(&cache_path).map_err(|e| format!("Failed to read Agents cache {cache_path:?}: {e}"))?;
  let (frontmatter, content) = parse_doc_frontmatter(&content);
  validate_doc_frontmatter(&cache_path.to_string_lossy(), &frontmatter)?;
  let byte_len = content.len();
  let line_len = content.lines().count();

  println!("{} {}", "Agent file:".dimmed(), cache_path.to_string_lossy().cyan());
  println!(
    "{} {} bytes, {} lines",
    "Agent length:".dimmed(),
    byte_len.to_string().cyan(),
    line_len.to_string().cyan()
  );

  let cache_display = cache_path.to_string_lossy().to_string();
  handle_read_content(ReadRenderOptions {
    display_title: frontmatter.title.as_deref().unwrap_or("Agents.md"),
    display_path: &cache_display,
    command_hint: "agents",
    no_match_error: "No heading matched in Agents.md. Run 'cr docs agents' to list available headings.",
    content: &content,
    heading_queries,
    include_subheadings,
    full,
    with_lines,
  })
}

fn handle_read(
  filename: &str,
  heading_queries: &[String],
  include_subheadings: bool,
  full: bool,
  with_lines: bool,
  module_filter: Option<&str>,
) -> Result<(), String> {
  let guide_docs = collect_docs_for_query(module_filter)?;
  let doc = find_doc_by_query(&guide_docs, filename)?;
  let read_full = full || heading_queries.is_empty();
  let result = handle_read_content(ReadRenderOptions {
    display_title: &doc.display_title(),
    display_path: &doc.path,
    command_hint: "read",
    no_match_error: "No section matched in document.",
    content: &doc.content,
    heading_queries,
    include_subheadings,
    full: read_full,
    with_lines,
  });

  if result.is_err() && !heading_queries.is_empty() {
    return Err(format!(
      "No section matched: {}. Use 'cr docs sections {filename}' to list available headings.",
      heading_queries.join(", ")
    ));
  }

  result
}

fn handle_read_lines(filename: &str, start: usize, lines_to_read: usize, module_filter: Option<&str>) -> Result<(), String> {
  let guide_docs = collect_docs_for_query(module_filter)?;
  let doc = find_doc_by_query(&guide_docs, filename)?;

  let all_lines: Vec<&str> = doc.content.lines().collect();
  let total_lines = all_lines.len();
  let end = (start + lines_to_read).min(total_lines);

  println!("{} ({})", doc.display_title().cyan().bold(), doc.path.dimmed());
  println!("{}", "=".repeat(60).dimmed());

  for (idx, line) in all_lines[start..end].iter().enumerate() {
    let line_num = start + idx + 1;
    println!("{} {}", format!("{line_num:4}:").dimmed(), line);
  }

  println!();
  println!(
    "{}",
    format!("Lines {}-{} of {} (total {} lines)", start + 1, end, total_lines, total_lines).dimmed()
  );

  if end < total_lines {
    let remaining = total_lines - end;
    println!(
      "{}",
      format!("More content available ({remaining} lines remaining). Next start line: {end}.").yellow()
    );
  } else {
    println!("{}", "End of document.".green());
  }

  if command_guidance_enabled() {
    println!(
      "{}",
      "Tip: Use -s <start> -n <lines> to read specific range (e.g., 'cr docs read-lines file.md -s 20 -n 30')".dimmed()
    );
  }

  Ok(())
}

fn handle_list(module_filter: Option<&str>) -> Result<(), String> {
  let guide_docs = collect_docs_for_query(module_filter)?;
  let header = match module_filter {
    Some(module) => format!("Available Documentation Files for module {module}:"),
    None => "Available Documentation Files for calcit:".to_string(),
  };

  println!("{}", header.bold());

  let mut docs: Vec<&GuideDoc> = guide_docs.iter().collect();
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
  if command_guidance_enabled() {
    println!("{}", "Use 'cr docs sections <filename>' to list headings in a document".dimmed());
    println!(
      "{}",
      "    'cr docs read <filename> <heading-keyword>' to read matched sections".dimmed()
    );
    println!("{}", "    'cr docs read <filename>' to read the full document".dimmed());
    println!(
      "{}",
      "    'cr docs read-lines <filename> -s <start> -n <lines>' for line-based reading".dimmed()
    );
    println!("{}", "    'cr docs search <keyword>' to search content".dimmed());
  }

  Ok(())
}

#[cfg(test)]
#[path = "docs_tests.rs"]
mod tests;

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
        block_start_line = idx + 1;
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

fn ensure_runtime_initialized() {
  static RUNTIME_INIT: Once = Once::new();
  RUNTIME_INIT.call_once(|| {
    calcit::builtins::effects::init_effects_states();
    #[cfg(not(target_arch = "wasm32"))]
    crate::injection::inject_platform_apis();
  });
}

fn module_folder() -> Result<PathBuf, String> {
  let home = std::env::var("HOME").map_err(|_| "Unable to get HOME environment variable".to_string())?;
  Ok(Path::new(&home).join(".config/calcit/modules/"))
}

fn collect_check_md_module_paths(entry: &str, deps: &[String]) -> Result<Vec<String>, String> {
  let resolved_entry = calcit::resolve_snapshot_path_alias(Path::new(entry));
  let mut content =
    fs::read_to_string(&resolved_entry).map_err(|e| format!("Failed to read entry file '{}': {e}", resolved_entry.display()))?;
  util::string::strip_shebang(&mut content);
  let data = cirru_edn::parse(&content).map_err(|e| format!("Failed to parse entry file '{}': {e}", resolved_entry.display()))?;

  // Extract configs.modules directly from the raw EDN to avoid loading the full snapshot.
  // This is necessary because old-format snapshots (with %{} :Expr code entries) cannot
  // be deserialized via load_snapshot_data, but we only need the module list here.
  let module_paths_from_entry: Vec<String> = extract_modules_from_edn(&data).unwrap_or_default();

  let mut module_paths = module_paths_from_entry;
  module_paths.extend(deps.iter().cloned());

  let mut seen_modules: HashSet<String> = HashSet::new();
  module_paths.retain(|module_path| seen_modules.insert(module_path.to_owned()));
  Ok(module_paths)
}

/// Extract the `configs.modules` list from an EDN snapshot value without fully
/// deserializing the snapshot. This tolerates old-format entries (e.g. `%{} :Expr`)
/// that `load_snapshot_data` cannot handle.
fn extract_modules_from_edn(data: &cirru_edn::Edn) -> Option<Vec<String>> {
  use cirru_edn::Edn;

  // Both old and new snapshot formats use a top-level Map or Record.
  let get_field = |edn: &Edn, key: &str| -> Option<Edn> {
    match edn {
      Edn::Map(map) => map.tag_get(key).cloned(),
      Edn::Record(record) => record.pairs.iter().find(|(k, _)| k.ref_str() == key).map(|(_, v)| v.clone()),
      _ => None,
    }
  };

  let configs = get_field(data, "configs")?;
  let modules_edn = get_field(&configs, "modules")?;

  match modules_edn {
    Edn::List(list) => {
      let paths: Vec<String> = list
        .0
        .iter()
        .filter_map(|item| match item {
          Edn::Str(s) => Some(s.to_string()),
          _ => None,
        })
        .collect();
      Some(paths)
    }
    _ => None,
  }
}

pub(crate) fn load_entry_snapshot_for_check_md(entry: &str) -> Result<snapshot::Snapshot, String> {
  let resolved_entry = calcit::resolve_snapshot_path_alias(Path::new(entry));
  let mut content =
    fs::read_to_string(&resolved_entry).map_err(|e| format!("Failed to read entry file '{}': {e}", resolved_entry.display()))?;
  util::string::strip_shebang(&mut content);
  let data = cirru_edn::parse(&content).map_err(|e| format!("Failed to parse entry file '{}': {e}", resolved_entry.display()))?;
  snapshot::load_snapshot_data(&data, &resolved_entry.display().to_string())
}

fn load_shared_files_for_check_md(entry: &str, deps: &[String]) -> Result<HashMap<String, snapshot::FileInSnapShot>, String> {
  ensure_runtime_initialized();

  let entry_path = PathBuf::from(entry);
  let base_dir = entry_path.parent().unwrap_or(Path::new("."));
  let module_folder = module_folder()?;
  let module_paths = collect_check_md_module_paths(entry, deps)?;

  let mut shared_files: HashMap<String, snapshot::FileInSnapShot> = HashMap::new();

  for module_path in &module_paths {
    let module_data = calcit::load_module(module_path, base_dir, &module_folder)?;
    for (k, v) in &module_data.files {
      if shared_files.contains_key(k) {
        return Err(format!("namespace `{k}` already exists when loading module `{module_path}`"));
      }
      shared_files.insert(k.to_owned(), v.to_owned());
    }
  }

  let core_snapshot = calcit::load_core_snapshot()?;
  for (k, v) in core_snapshot.files {
    shared_files.insert(k.to_owned(), v);
  }

  Ok(shared_files)
}

fn build_entries_from_snapshot(snapshot: &snapshot::Snapshot) -> Result<ProgramEntries, String> {
  let config_init = snapshot.configs.init_fn.to_string();
  let config_reload = snapshot.configs.reload_fn.to_string();
  let (init_ns, init_def) = util::string::extract_ns_def(&config_init)?;
  let (reload_ns, reload_def) = util::string::extract_ns_def(&config_reload)?;

  Ok(ProgramEntries {
    init_fn: Arc::from(config_init),
    reload_fn: Arc::from(config_reload),
    init_def: Arc::from(init_def),
    init_ns: Arc::from(init_ns),
    reload_ns: Arc::from(reload_ns),
    reload_def: Arc::from(reload_def),
  })
}

fn prepare_program_for_snippet(
  entry: &str,
  shared_files: &HashMap<String, snapshot::FileInSnapShot>,
  code: &str,
) -> Result<ProgramEntries, String> {
  ensure_runtime_initialized();

  // `check-md` runs many snippets in one process; clear stale runtime/compiled
  // state so each block is evaluated from the freshly built app.main snapshot.
  program::clear_runtime_caches_for_reload(Arc::from("app.main"), Arc::from("app.main"), true)?;

  let mut snapshot = match load_entry_snapshot_for_check_md(entry) {
    Ok(entry_snapshot) => entry_snapshot,
    Err(e) => {
      eprintln!("check-md: failed to load entry `{entry}`: {e}");
      snapshot::Snapshot::default()
    }
  };
  let main_file = snapshot::create_file_from_snippet(code)?;

  for (k, v) in shared_files {
    snapshot.files.insert(k.to_owned(), v.to_owned());
  }
  // Insert snippet last so loaded modules cannot overwrite the markdown block under test.
  snapshot.files.insert(String::from("app.main"), main_file);

  {
    let mut prgm = program::PROGRAM_CODE_DATA.write().map_err(|_| "open program data".to_string())?;
    *prgm = program::extract_program_data(&snapshot)?;
  }

  let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
  runner::preprocess::ensure_ns_def_compiled(
    calcit::calcit::CORE_NS,
    calcit::calcit::BUILTIN_IMPLS_ENTRY,
    check_warnings,
    &CallStackList::default(),
  )
  .map_err(|e| e.msg)?;

  build_entries_from_snapshot(&snapshot)
}

fn run_check_only_in_process(entries: &ProgramEntries) -> Result<(), String> {
  let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);

  let compile_entry_def = |ns: &Arc<str>, def: &Arc<str>| {
    runner::preprocess::ensure_ns_def_compiled(ns, def, check_warnings, &CallStackList::default()).map_err(|failure| failure.msg)
  };

  // `prepare_program_for_snippet` always injects the markdown block as `app.main`.
  // Prefer checking those defs so entry init/reload from the loaded project do not
  // shadow snippet validation when they compile successfully.
  let snippet_targets: Vec<(Arc<str>, Arc<str>)> = {
    let program_data = program::PROGRAM_CODE_DATA.read().map_err(|_| "open program data".to_string())?;
    program_data
      .get("app.main")
      .map(|file_data| {
        file_data
          .defs
          .keys()
          .map(|def_name| (Arc::from("app.main"), def_name.to_owned()))
          .collect()
      })
      .unwrap_or_default()
  };

  if !snippet_targets.is_empty() {
    for (ns_name, def_name) in snippet_targets {
      compile_entry_def(&ns_name, &def_name).map_err(|e| format!("compile {ns_name}/{def_name}: {e}"))?;
    }
  } else {
    let entry_check =
      compile_entry_def(&entries.init_ns, &entries.init_def).and_then(|_| compile_entry_def(&entries.reload_ns, &entries.reload_def));

    if let Err(entry_err) = entry_check {
      if entry_err.contains("unknown ns/def in program") {
        let targets: Vec<(Arc<str>, Arc<str>)> = {
          let program_data = program::PROGRAM_CODE_DATA.read().map_err(|_| "open program data".to_string())?;
          let mut pairs: Vec<(Arc<str>, Arc<str>)> = Vec::new();
          for (ns_name, file_data) in &*program_data {
            if ns_name.starts_with("app.") {
              for def_name in file_data.defs.keys() {
                pairs.push((ns_name.to_owned(), def_name.to_owned()));
              }
            }
          }
          pairs
        };

        if targets.is_empty() {
          return Err(entry_err);
        }

        for (ns_name, def_name) in targets {
          compile_entry_def(&ns_name, &def_name)?;
        }
      } else {
        return Err(entry_err);
      }
    }
  }

  let warnings = check_warnings.borrow();
  if !warnings.is_empty() {
    let mut lines = vec![format!("Warnings: ({} warnings)", warnings.len())];
    lines.extend(warnings.iter().map(|w| w.to_string()));
    return Err(lines.join("\n"));
  }

  Ok(())
}

fn run_eval_in_process(entries: &ProgramEntries) -> Result<(), String> {
  match calcit::run_program_with_docs(entries.init_ns.to_owned(), entries.init_def.to_owned(), &[]) {
    Ok(_) => Ok(()),
    Err(e) => {
      let mut lines: Vec<String> = vec![];
      if !e.warnings.is_empty() {
        lines.push(format!("Warnings: ({} warnings)", e.warnings.len()));
        lines.extend(e.warnings.iter().map(|w| w.to_string()));
      }
      lines.push(format!("Error: {}", e.msg));
      Err(lines.join("\n"))
    }
  }
}

fn run_parse_only_in_process(code: &str) -> Result<(), String> {
  cirru_parser::parse(code)
    .map(|_| ())
    .map_err(|e| format!("Error: Failed to parse code snippet: {e}"))
}

fn handle_check_md(file_path: &str, entry: &str, deps: &[String]) -> Result<(), String> {
  let content = fs::read_to_string(file_path).map_err(|e| format!("Failed to read file '{file_path}': {e}"))?;

  let blocks = extract_cirru_blocks(&content);

  if blocks.is_empty() {
    println!("{}", "No cirru code blocks found in the file.".yellow());
    return Ok(());
  }

  if !Path::new(entry).exists() {
    return Err(format!(
      "Entry file '{entry}' not found. Use -d to specify a valid entry .cirru file."
    ));
  }

  let shared_files = load_shared_files_for_check_md(entry, deps)?;

  println!("{} {}", "Checking".bold(), file_path.cyan());
  println!("{}", "-".repeat(60).dimmed());

  let mut passed = 0;
  let mut failed = 0;
  let total = blocks.len();
  let run_count = blocks.iter().filter(|(_, mode, _)| *mode == CirruCheckMode::Run).count();
  let no_run_count = blocks.iter().filter(|(_, mode, _)| *mode == CirruCheckMode::NoRun).count();
  let no_check_count = blocks.iter().filter(|(_, mode, _)| *mode == CirruCheckMode::NoCheck).count();

  if run_count < no_check_count {
    println!(
      "{}",
      format!(
        "Tip: check-mode balance is skewed (cirru: {run_count}, cirru.no-run: {no_run_count}, cirru.no-check: {no_check_count}). Prefer `cirru` first, then `cirru.no-run`, and use `cirru.no-check` only when necessary."
      )
      .yellow()
    );
    println!("{}", "-".repeat(60).dimmed());
  }

  for (line_num, mode, code) in &blocks {
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

    let check_result = match mode {
      CirruCheckMode::Run => prepare_program_for_snippet(entry, &shared_files, code).and_then(|entries| run_eval_in_process(&entries)),
      CirruCheckMode::NoRun => {
        prepare_program_for_snippet(entry, &shared_files, code).and_then(|entries| run_check_only_in_process(&entries))
      }
      CirruCheckMode::NoCheck => run_parse_only_in_process(code),
    };

    if check_result.is_ok() {
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
      let stderr = check_result
        .as_ref()
        .err()
        .cloned()
        .unwrap_or_else(|| "Error: Unknown error".to_string());
      println!(
        "  {} L{}: {}{}{}",
        "✗".red(),
        format!("{line_num}").yellow(),
        mode_label,
        preview,
        preview_suffix
      );

      for line in stderr.lines() {
        if !line.trim().is_empty() {
          println!("    {}", line.red());
        }
      }
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
