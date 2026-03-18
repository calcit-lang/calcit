use colored::Colorize;
use std::sync::atomic::{AtomicBool, Ordering};

static TIPS_SUPPRESSED: AtomicBool = AtomicBool::new(false);
static TIPS_FULL: AtomicBool = AtomicBool::new(false);

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
}

/// Default suggestion for Cirru editing in CLI: prefer one-liner and JSON when helpful
pub fn tip_prefer_oneliner_json(show_json: bool) -> Vec<String> {
  let mut tips = Vec::new();
  tips.push(format!(
    "Prefer {} to avoid indentation issues; for messy structures, use {} to inspect JSON format",
    "-e 'one-liner'".yellow(),
    "-j".yellow()
  ));
  if !show_json {
    tips.push(format!("add {} flag to also output JSON format", "-j".yellow()));
  }
  tips
}

/// Tip for discouraging root-level edits when path is empty during editing
pub fn tip_root_edit(path_is_empty: bool) -> Option<String> {
  if path_is_empty {
    Some(
      "Editing root path; prefer cr edit def --overwrite -f <file> for whole-definition rewrites, and keep tree replace for intentional root-node surgery"
        .to_string(),
    )
  } else {
    None
  }
}

/// Tips for `cr query ns` when listing namespaces
pub fn tip_query_ns_list(include_deps: bool) -> Vec<String> {
  let mut tips = Vec::new();
  tips.push("Use `cr query ns <namespace>` to show namespace details.".to_string());
  tips.push("Use `cr query defs <namespace>` to list definitions.".to_string());
  if !include_deps {
    tips.push("Use `--deps` to include dependency and core namespaces.".to_string());
  }
  tips
}

/// Tips for `cr query defs` when showing definitions list
pub fn tip_query_defs_list() -> Vec<String> {
  vec!["Use `cr query peek <ns/def>` for signature, `cr query def <ns/def>` for full code.".to_string()]
}
