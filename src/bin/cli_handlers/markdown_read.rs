use colored::Colorize;

use super::tips::command_guidance_enabled;

pub struct RenderMarkdownOptions<'a> {
  pub include_subheadings: bool,
  pub full: bool,
  pub with_lines: bool,
  pub command_prefix: &'a str,
  pub with_file_option: bool,
  pub no_headings_message: &'a str,
  pub print_full_when_no_headings: bool,
  pub no_match_error: &'a str,
}

#[derive(Debug, Clone)]
pub struct HeadingSection {
  pub line: usize,
  pub level: usize,
  pub title: String,
  pub content_start: usize,
  pub content_end: usize,
}

pub fn extract_markdown_headings(content: &str) -> Vec<(usize, usize, String)> {
  let mut results: Vec<(usize, usize, String)> = vec![];
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
    let after_hash = trimmed.chars().nth(level);
    if level == 0 || after_hash != Some(' ') {
      continue;
    }

    let title = trimmed[level + 1..].trim();
    if title.is_empty() {
      continue;
    }

    results.push((idx + 1, level, title.to_owned()));
  }

  results
}

pub fn build_heading_sections(content: &str) -> Vec<HeadingSection> {
  let lines: Vec<&str> = content.lines().collect();
  let total_lines = lines.len();
  let headings = extract_markdown_headings(content);
  let mut sections: Vec<HeadingSection> = vec![];

  for (idx, (line, level, title)) in headings.iter().enumerate() {
    let mut content_end = total_lines;
    for (next_line, next_level, _) in headings.iter().skip(idx + 1) {
      if *next_level <= *level {
        content_end = next_line.saturating_sub(1);
        break;
      }
    }

    sections.push(HeadingSection {
      line: *line,
      level: *level,
      title: title.to_owned(),
      content_start: line + 1,
      content_end,
    });
  }

  sections
}

pub fn collapse_nested_sections(indices: &[usize], sections: &[HeadingSection]) -> Vec<usize> {
  let mut sorted: Vec<usize> = indices.to_vec();
  sorted.sort_by_key(|idx| sections[*idx].line);

  let mut kept: Vec<usize> = vec![];
  for idx in sorted {
    let current = &sections[idx];
    let covered_by_parent = kept.iter().any(|kept_idx| {
      let parent = &sections[*kept_idx];
      parent.line <= current.line && parent.content_end >= current.content_end
    });
    if !covered_by_parent {
      kept.push(idx);
    }
  }

  kept
}

pub fn select_heading_indices(heading_queries: &[String], sections: &[HeadingSection]) -> (Vec<usize>, Vec<String>) {
  let mut selected_indices: Vec<usize> = vec![];
  let mut unmatched: Vec<String> = vec![];

  for query in heading_queries {
    let query_lower = query.to_lowercase();
    let mut found = false;
    for (idx, section) in sections.iter().enumerate() {
      if section.title.to_lowercase().contains(&query_lower) {
        found = true;
        if !selected_indices.contains(&idx) {
          selected_indices.push(idx);
        }
      }
    }
    if !found {
      unmatched.push(query.to_owned());
    }
  }

  (selected_indices, unmatched)
}

pub fn print_headings_list(sections: &[HeadingSection], with_lines: bool) {
  println!("{}", "Headings:".bold());
  for section in sections {
    if with_lines {
      println!(
        "  {}\t{}{}",
        format!("L{}", section.line).dimmed(),
        "#".repeat(section.level).dimmed(),
        format_args!(" {}", section.title)
      );
    } else {
      println!("  {} {}", "#".repeat(section.level).dimmed(), section.title);
    }
  }
}

pub fn print_selected_sections(
  lines: &[&str],
  sections: &[HeadingSection],
  selected_indices: &[usize],
  include_subheadings: bool,
  with_lines: bool,
) {
  for idx in selected_indices {
    let section = &sections[*idx];
    if with_lines {
      println!(
        "{} {} ({})",
        "#".repeat(section.level).cyan().bold(),
        section.title.cyan().bold(),
        format!("L{}", section.line).dimmed()
      );
    } else {
      println!("{} {}", "#".repeat(section.level).cyan().bold(), section.title.cyan().bold());
    }
    println!("{}", "-".repeat(60).dimmed());

    let section_end = if include_subheadings {
      section.content_end
    } else if *idx + 1 < sections.len() {
      sections[*idx + 1].line.saturating_sub(1)
    } else {
      lines.len()
    };

    if section.content_start > section_end || section.content_start > lines.len() {
      println!("{}", "(empty section)".dimmed());
    } else {
      let start = section.content_start;
      let end = section_end.min(lines.len());
      for line in &lines[start - 1..end] {
        println!("{line}");
      }
    }
    println!();
  }
}

pub fn print_markdown_read_tips(command_prefix: &str, with_file_option: bool) {
  if !command_guidance_enabled() {
    return;
  }

  println!(
    "{}",
    format!("Tip: Use '{command_prefix} <heading-keyword> [more-keywords...]' for fuzzy heading matching.").dimmed()
  );
  println!("{}", "     Use '--no-subheadings' to show only direct section content.".dimmed());
  if with_file_option {
    println!(
      "{}",
      "     Use '--full' to print the whole file (works with --file <path> too).".dimmed()
    );
  } else {
    println!(
      "{}",
      format!("     Use '{command_prefix} --full' to print the whole file.").dimmed()
    );
  }
  println!(
    "{}",
    format!("     Use '{command_prefix} --with-lines' to show heading line numbers.").dimmed()
  );
}

pub fn render_markdown_sections(content: &str, heading_queries: &[String], options: RenderMarkdownOptions<'_>) -> Result<(), String> {
  if options.full {
    println!("{content}");
    return Ok(());
  }

  let lines: Vec<&str> = content.lines().collect();
  let sections = build_heading_sections(content);

  if sections.is_empty() {
    println!("{}", options.no_headings_message.yellow());
    if options.print_full_when_no_headings {
      println!("\n{content}");
    }
    return Ok(());
  }

  if heading_queries.is_empty() {
    print_headings_list(&sections, options.with_lines);
    println!();
    print_markdown_read_tips(options.command_prefix, options.with_file_option);
    return Ok(());
  }

  let (selected_indices, unmatched) = select_heading_indices(heading_queries, &sections);

  if selected_indices.is_empty() {
    return Err(options.no_match_error.to_string());
  }

  let selected_indices = if options.include_subheadings {
    collapse_nested_sections(&selected_indices, &sections)
  } else {
    selected_indices
  };

  print_selected_sections(
    &lines,
    &sections,
    &selected_indices,
    options.include_subheadings,
    options.with_lines,
  );

  if !unmatched.is_empty() {
    println!("{}", format!("No match for heading query: {}", unmatched.join(", ")).yellow());
  }

  Ok(())
}
