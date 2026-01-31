use colored::Colorize;

/// Simple helper to collect and print tips consistently across commands
pub struct Tips {
  items: Vec<String>,
}

impl Tips {
  pub fn new() -> Self {
    Tips { items: Vec::new() }
  }

  pub fn add(&mut self, tip: impl Into<String>) {
    self.items.push(tip.into());
  }

  #[allow(dead_code)]
  pub fn add_if(&mut self, cond: bool, tip: impl Into<String>) {
    if cond {
      self.add(tip);
    }
  }

  pub fn append(&mut self, tips: Vec<String>) {
    for t in tips {
      self.items.push(t);
    }
  }

  /// Print collected tips using a consistent format
  pub fn print(&self) {
    if self.items.is_empty() {
      return;
    }
    println!("{}: {}", "Tips".blue().bold(), self.items.join("; "));
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
    Some("Editing root path; prefer local updates to avoid unintended changes".to_string())
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
