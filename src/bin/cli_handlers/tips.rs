use colored::Colorize;
use std::sync::atomic::{AtomicBool, Ordering};

static TIPS_SUPPRESSED: AtomicBool = AtomicBool::new(false);
static TIPS_FULL: AtomicBool = AtomicBool::new(false);
static COMMAND_GUIDANCE_SUPPRESSED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipsLevel {
  Minimal,
  Full,
  None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TipPriority {
  Normal,
  High,
}

impl TipsLevel {
  fn parse(raw: &str) -> Option<Self> {
    match raw {
      "minimal" => Some(Self::Minimal),
      "full" => Some(Self::Full),
      "none" => Some(Self::None),
      _ => None,
    }
  }
}

/// Suppress all tips output globally
pub fn suppress_tips() {
  TIPS_SUPPRESSED.store(true, Ordering::Relaxed);
  TIPS_FULL.store(false, Ordering::Relaxed);
}

pub fn suppress_command_guidance() {
  COMMAND_GUIDANCE_SUPPRESSED.store(true, Ordering::Relaxed);
  suppress_tips();
}

pub fn command_guidance_enabled() -> bool {
  !COMMAND_GUIDANCE_SUPPRESSED.load(Ordering::Relaxed)
}

pub fn set_tips_level(raw: &str) -> Result<(), String> {
  let level =
    TipsLevel::parse(raw).ok_or_else(|| format!("Invalid --tips-level value '{raw}'. Expected one of: minimal, full, none"))?;

  match level {
    TipsLevel::Minimal => {
      TIPS_SUPPRESSED.store(false, Ordering::Relaxed);
      TIPS_FULL.store(false, Ordering::Relaxed);
    }
    TipsLevel::Full => {
      TIPS_SUPPRESSED.store(false, Ordering::Relaxed);
      TIPS_FULL.store(true, Ordering::Relaxed);
    }
    TipsLevel::None => suppress_tips(),
  }

  Ok(())
}

/// Simple helper to collect and print tips consistently across commands
pub struct Tips {
  items: Vec<(TipPriority, String)>,
}

impl Tips {
  pub fn new() -> Self {
    Tips { items: Vec::new() }
  }

  pub fn add(&mut self, tip: impl Into<String>) {
    self.items.push((TipPriority::Normal, tip.into()));
  }

  pub fn add_with_priority(&mut self, priority: TipPriority, tip: impl Into<String>) {
    self.items.push((priority, tip.into()));
  }

  #[allow(dead_code)]
  pub fn add_if(&mut self, cond: bool, tip: impl Into<String>) {
    if cond {
      self.add(tip);
    }
  }

  pub fn append(&mut self, tips: Vec<String>) {
    for t in tips {
      self.items.push((TipPriority::Normal, t));
    }
  }

  /// Print collected tips using a consistent format
  pub fn print(&self) {
    if self.items.is_empty() || TIPS_SUPPRESSED.load(Ordering::Relaxed) {
      return;
    }

    if TIPS_FULL.load(Ordering::Relaxed) {
      let all = self
        .items
        .iter()
        .map(|(_, message)| message.as_str())
        .collect::<Vec<_>>()
        .join("; ");
      println!("{}: {}", "Tips".blue().bold(), all);
      return;
    }

    if let Some((_, message)) = self.items.iter().find(|(priority, _)| *priority == TipPriority::High) {
      println!("{}: {}", "Tips".blue().bold(), message);
    }
  }

  /// Print tips without ANSI styling so they can extend a Markdown document.
  pub fn print_markdown(&self, to_stderr: bool) {
    if self.items.is_empty() || TIPS_SUPPRESSED.load(Ordering::Relaxed) {
      return;
    }

    let messages = if TIPS_FULL.load(Ordering::Relaxed) {
      self.items.iter().map(|(_, message)| message.as_str()).collect::<Vec<_>>()
    } else {
      self
        .items
        .iter()
        .find(|(priority, _)| *priority == TipPriority::High)
        .map(|(_, message)| vec![message.as_str()])
        .unwrap_or_default()
    };
    if messages.is_empty() {
      return;
    }
    let mut lines = vec!["## Tips".to_owned(), String::new()];
    lines.extend(messages.into_iter().map(|message| format!("- {message}")));
    let output = lines.join("\n");
    if to_stderr {
      eprintln!("{output}");
    } else {
      println!("{output}");
    }
  }
}

pub fn tip_prefer_oneliner_json_markdown(show_json: bool) -> Vec<String> {
  let mut tips = vec!["Prefer `--code 'one-liner'` to avoid indentation issues; use `--json` to inspect a JSON AST.".to_owned()];
  if !show_json {
    tips.push("Add `--json` to append a fenced JSON AST.".to_owned());
  }
  tips
}

/// Tip for discouraging root-level edits when path is empty during editing
pub fn tip_root_edit(path_is_empty: bool) -> Option<String> {
  if path_is_empty {
    Some(
      "Editing root path; prefer calcit edit def --overwrite --file <file> for whole-definition rewrites, and keep tree replace for intentional root-node surgery"
        .to_string(),
    )
  } else {
    None
  }
}
