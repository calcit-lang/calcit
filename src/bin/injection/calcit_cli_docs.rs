//! Docs builtins: read guide sections, agents cache, heading lists.

use calcit::calcit::{Calcit, CalcitErr};
use calcit::call_stack::CallStackList;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::calcit_cli_args::resolve_cli_args;
use super::calcit_cli_specs::{DOCS_AGENTS, DOCS_READ, DOCS_SECTIONS};

const AGENTS_CACHE: &str = ".config/calcit/Agents.md";

pub fn docs_agents(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/docs-agents", &xs, DOCS_AGENTS)?;
  let headings_csv = args.optional_string("headings");
  let full = args.bool("full")?;
  let path = agents_cache_path()?;
  let content =
    fs::read_to_string(&path).map_err(|e| CalcitErr::from(format!("docs-agents: failed to read {}: {e}", path.display())))?;
  let body = strip_frontmatter(&content);
  let queries: Vec<String> = headings_csv
    .map(|csv| csv.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
    .unwrap_or_default();
  Ok(Calcit::Str(Arc::from(render_markdown(&body, &queries, full)?)))
}

pub fn docs_read(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/docs-read", &xs, DOCS_READ)?;
  let filename = args.string("filename")?;
  let headings_csv = args.optional_string("headings");
  let full = args.bool("full")?;
  let path = find_doc_path(&filename)?;
  let content = fs::read_to_string(&path).map_err(|e| CalcitErr::from(format!("docs-read: failed to read {}: {e}", path.display())))?;
  let body = strip_frontmatter(&content);
  let queries: Vec<String> = headings_csv
    .map(|csv| csv.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
    .unwrap_or_default();
  let read_full = full || queries.is_empty();
  Ok(Calcit::Str(Arc::from(render_markdown(&body, &queries, read_full)?)))
}

pub fn docs_sections(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/docs-sections", &xs, DOCS_SECTIONS)?;
  let filename = args.string("filename")?;
  let path = find_doc_path(&filename)?;
  let content =
    fs::read_to_string(&path).map_err(|e| CalcitErr::from(format!("docs-sections: failed to read {}: {e}", path.display())))?;
  let body = strip_frontmatter(&content);
  let headings = extract_markdown_headings(&body);
  let mut lines = Vec::new();
  for (line, level, title) in headings {
    lines.push(format!("L{line} {} {}", "#".repeat(level), title));
  }
  Ok(Calcit::Str(Arc::from(lines.join("\n"))))
}

fn agents_cache_path() -> Result<PathBuf, CalcitErr> {
  let home = std::env::var("HOME").map_err(|_| CalcitErr::from("docs-agents: HOME not set".to_string()))?;
  Ok(Path::new(&home).join(AGENTS_CACHE))
}

fn guidebook_dir() -> Result<PathBuf, CalcitErr> {
  if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
    let dev_docs = Path::new(&manifest).join("docs");
    if dev_docs.is_dir() {
      return Ok(dev_docs);
    }
  }
  let home = std::env::var("HOME").map_err(|_| CalcitErr::from("docs: HOME not set".to_string()))?;
  let docs_dir = Path::new(&home).join(".config/calcit/docs");
  if docs_dir.is_dir() {
    return Ok(docs_dir);
  }
  Err(CalcitErr::from(format!(
    "docs: guidebook directory not found (tried CARGO_MANIFEST_DIR/docs and {})",
    docs_dir.display()
  )))
}

fn find_doc_path(query: &str) -> Result<PathBuf, CalcitErr> {
  let root = guidebook_dir()?;
  let query_lower = query.to_lowercase();
  let mut best: Option<(usize, PathBuf)> = None;
  collect_doc_matches(&root, &root, &query_lower, &mut best)?;
  best
    .map(|(_, p)| p)
    .ok_or_else(|| CalcitErr::from(format!("docs: document `{query}` not found under {}", root.display())))
}

fn collect_doc_matches(base: &Path, dir: &Path, query_lower: &str, best: &mut Option<(usize, PathBuf)>) -> Result<(), CalcitErr> {
  for entry in fs::read_dir(dir).map_err(|e| CalcitErr::from(format!("docs: read dir {}: {e}", dir.display())))? {
    let entry = entry.map_err(|e| CalcitErr::from(format!("docs: read entry: {e}")))?;
    let path = entry.path();
    if path.is_dir() {
      if path.file_name().and_then(|s| s.to_str()).is_some_and(|n| n.starts_with('.')) {
        continue;
      }
      collect_doc_matches(base, &path, query_lower, best)?;
    } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
      let rel = path.strip_prefix(base).unwrap_or(&path).to_string_lossy().to_string();
      let rel_lower = rel.to_lowercase();
      let score = if rel_lower == query_lower {
        1000
      } else if Path::new(&rel_lower).file_name().and_then(|s| s.to_str()) == Some(query_lower) {
        800
      } else if rel_lower.contains(query_lower) {
        400
      } else {
        0
      };
      if score > 0 {
        let replace = best.as_ref().map(|(s, _)| score > *s).unwrap_or(true);
        if replace {
          *best = Some((score, path));
        }
      }
    }
  }
  Ok(())
}

fn strip_frontmatter(raw: &str) -> String {
  if !raw.starts_with("---\n") {
    return raw.to_string();
  }
  let mut lines = raw.lines();
  lines.next();
  while let Some(line) = lines.next() {
    if line.trim() == "---" {
      return lines.collect::<Vec<_>>().join("\n");
    }
  }
  raw.to_string()
}

fn extract_markdown_headings(content: &str) -> Vec<(usize, usize, String)> {
  let mut results = Vec::new();
  let mut in_fence = false;
  for (idx, line) in content.lines().enumerate() {
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") {
      in_fence = !in_fence;
      continue;
    }
    if in_fence || !trimmed.starts_with('#') {
      continue;
    }
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if level == 0 || trimmed.chars().nth(level) != Some(' ') {
      continue;
    }
    let title = trimmed[level + 1..].trim();
    if !title.is_empty() {
      results.push((idx + 1, level, title.to_string()));
    }
  }
  results
}

fn render_markdown(content: &str, heading_queries: &[String], full: bool) -> Result<String, CalcitErr> {
  if full || heading_queries.is_empty() {
    return Ok(content.to_string());
  }

  let lines: Vec<&str> = content.lines().collect();
  let headings = extract_markdown_headings(content);
  let mut selected = Vec::new();
  for query in heading_queries {
    let q = query.to_lowercase();
    for (idx, (_, _, title)) in headings.iter().enumerate() {
      if title.to_lowercase().contains(&q) {
        selected.push(idx);
      }
    }
  }
  if selected.is_empty() {
    return Err(CalcitErr::from(format!("docs: no heading matched: {}", heading_queries.join(", "))));
  }

  let mut out = String::new();
  for idx in selected {
    let (start_line, level, title) = &headings[idx];
    let end_line = headings
      .iter()
      .skip(idx + 1)
      .find(|(_, lv, _)| *lv <= *level)
      .map(|(line, _, _)| line.saturating_sub(1))
      .unwrap_or(lines.len());

    out.push_str(&format!("{} {}\n", "#".repeat(*level), title));
    for line in &lines[*start_line..end_line.min(lines.len())] {
      out.push_str(line);
      out.push('\n');
    }
    out.push('\n');
  }
  Ok(out)
}
