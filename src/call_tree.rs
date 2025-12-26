//! Call tree analysis module for Calcit programs
//!
//! This module extracts and analyzes the call tree structure of a Calcit program,
//! starting from a specified entry point. It helps understand code dependencies
//! and identify unused definitions.

use crate::calcit::Calcit;
use crate::program::{PROGRAM_CODE_DATA, ProgramCodeData};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Represents a node in the call tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallTreeNode {
  /// Namespace of the definition
  pub ns: String,
  /// Definition name
  pub def: String,
  /// Full qualified name (ns/def)
  pub fqn: String,
  /// Documentation string if available
  #[serde(skip_serializing_if = "Option::is_none")]
  pub doc: Option<String>,
  /// Child calls (definitions this node calls)
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub calls: Vec<CallTreeNode>,
  /// Whether this node represents a circular reference
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  pub circular: bool,
  /// Whether this node was already expanded elsewhere (skip showing children again)
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  pub seen: bool,
  /// Source type: "project", "core", or "external"
  pub source: String,
}

/// Result of call tree analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallTreeResult {
  /// The entry point of the analysis
  pub entry: String,
  /// The call tree starting from entry
  pub tree: CallTreeNode,
  /// Statistics about the analysis
  pub stats: CallTreeStats,
  /// Unused definitions (only if requested)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub unused_definitions: Option<Vec<UnusedDefinition>>,
}

/// Statistics about the call tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallTreeStats {
  /// Total definitions reachable from entry
  pub reachable_count: usize,
  /// Number of circular references detected
  pub circular_count: usize,
  /// Number of project definitions
  pub project_defs: usize,
  /// Number of core/library definitions
  pub core_defs: usize,
  /// Maximum depth of the call tree
  pub max_depth: usize,
}

/// Represents an unused definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedDefinition {
  /// Namespace of the unused definition
  pub ns: String,
  /// Definition name
  pub def: String,
  /// Full qualified name
  pub fqn: String,
  /// Documentation if available
  #[serde(skip_serializing_if = "Option::is_none")]
  pub doc: Option<String>,
}

/// Configuration for call tree analysis
#[derive(Debug, Clone, Default)]
pub struct CallTreeConfig {
  /// Whether to include core/library calls
  pub include_core: bool,
  /// Maximum depth to traverse (0 = unlimited)
  pub max_depth: usize,
  /// Whether to analyze unused definitions
  pub show_unused: bool,
  /// Package name to filter project definitions
  pub package_name: Option<String>,
  /// Namespace prefix filter - only show definitions whose ns starts with this
  pub ns_prefix: Option<String>,
}

/// Call tree analyzer
pub struct CallTreeAnalyzer {
  config: CallTreeConfig,
  /// Track visited definitions to handle circular references
  visited: HashSet<String>,
  /// Track definitions that have been fully expanded (fqn -> had_children)
  expanded: HashMap<String, bool>,
  /// All reachable definitions
  reachable: HashSet<String>,
  /// Circular reference count
  circular_count: usize,
  /// Maximum depth reached
  max_depth: usize,
}

impl CallTreeAnalyzer {
  pub fn new(config: CallTreeConfig) -> Self {
    CallTreeAnalyzer {
      config,
      visited: HashSet::new(),
      expanded: HashMap::new(),
      reachable: HashSet::new(),
      circular_count: 0,
      max_depth: 0,
    }
  }

  /// Analyze the call tree starting from the given entry point
  pub fn analyze(&mut self, entry_ns: &str, entry_def: &str) -> Result<CallTreeResult, String> {
    let fqn = format!("{entry_ns}/{entry_def}");

    // Build the full call tree
    let mut tree = self.build_tree(entry_ns, entry_def, 0)?;

    // If ns_prefix specified, prune the tree to only include nodes that match the prefix
    // while keeping the ancestor paths to matching nodes
    if let Some(ref prefix) = self.config.ns_prefix {
      tree = prune_tree_by_ns_prefix(tree, prefix);
    }

    // Calculate statistics
    let program_code = PROGRAM_CODE_DATA.read().map_err(|e| format!("Failed to read program code: {e}"))?;

    // When filtered, recompute stats from the pruned tree to reflect the display
    let (reachable_count, project_defs, core_defs, circular_count, max_depth) = if self.config.ns_prefix.is_some() {
      compute_stats_from_tree(&tree, |ns| self.is_core_ns(ns))
    } else {
      let (project_defs, core_defs) = self.count_def_types(&program_code);
      (self.reachable.len(), project_defs, core_defs, self.circular_count, self.max_depth)
    };

    let stats = CallTreeStats {
      reachable_count,
      circular_count,
      project_defs,
      core_defs,
      max_depth,
    };

    // Analyze unused definitions if requested
    let unused_definitions = if self.config.show_unused {
      Some(self.find_unused_definitions(&program_code))
    } else {
      None
    };

    Ok(CallTreeResult {
      entry: fqn,
      tree,
      stats,
      unused_definitions,
    })
  }

  fn build_tree(&mut self, ns: &str, def: &str, depth: usize) -> Result<CallTreeNode, String> {
    let fqn = format!("{ns}/{def}");

    // Update max depth
    if depth > self.max_depth {
      self.max_depth = depth;
    }

    // Check max depth limit
    if self.config.max_depth > 0 && depth >= self.config.max_depth {
      return Ok(CallTreeNode {
        ns: ns.to_string(),
        def: def.to_string(),
        fqn: fqn.clone(),
        doc: None,
        calls: vec![],
        circular: false,
        seen: false,
        source: self.get_source_type(ns),
      });
    }

    // Check for circular reference (in current call path)
    if self.visited.contains(&fqn) {
      self.circular_count += 1;
      return Ok(CallTreeNode {
        ns: ns.to_string(),
        def: def.to_string(),
        fqn: fqn.clone(),
        doc: None,
        calls: vec![],
        circular: true,
        seen: false,
        source: self.get_source_type(ns),
      });
    }

    // Check if already expanded elsewhere - just mark as seen, don't expand again
    if let Some(&had_children) = self.expanded.get(&fqn) {
      return Ok(CallTreeNode {
        ns: ns.to_string(),
        def: def.to_string(),
        fqn: fqn.clone(),
        doc: None,
        calls: vec![],
        circular: false,
        // Only mark as seen if the original expansion had children (content was collapsed)
        seen: had_children,
        source: self.get_source_type(ns),
      });
    }

    // Mark as visited
    self.visited.insert(fqn.clone());
    self.reachable.insert(fqn.clone());

    let program_code = PROGRAM_CODE_DATA.read().map_err(|e| format!("Failed to read program code: {e}"))?;

    // Get the definition
    let (code_entry, doc) = match program_code.get(ns) {
      Some(file_data) => match file_data.defs.get(def) {
        Some(entry) => {
          let doc = if entry.doc.is_empty() { None } else { Some(entry.doc.to_string()) };
          (Some(&entry.code), doc)
        }
        None => (None, None),
      },
      None => (None, None),
    };

    // Extract calls from the code
    let mut calls = vec![];
    if let Some(code) = code_entry {
      let call_refs = self.extract_calls(code, ns);

      // Release the lock before recursive calls
      drop(program_code);

      for (call_ns, call_def) in call_refs {
        // Filter based on config
        if !self.config.include_core && self.is_core_ns(&call_ns) {
          continue;
        }

        // Skip self-recursion (tail recursion etc.) - not meaningful for external analysis
        if call_ns == ns && call_def == def {
          continue;
        }

        let child_tree = self.build_tree(&call_ns, &call_def, depth + 1)?;
        calls.push(child_tree);
      }
    } else {
      drop(program_code);
    }

    // Unmark from current path (allow revisiting in different branches)
    self.visited.remove(&fqn);

    // Mark as expanded, recording whether it had children
    self.expanded.insert(fqn.clone(), !calls.is_empty());

    Ok(CallTreeNode {
      ns: ns.to_string(),
      def: def.to_string(),
      fqn,
      doc,
      calls,
      circular: false,
      seen: false,
      source: self.get_source_type(ns),
    })
  }

  /// Extract all function/definition calls from a Calcit expression
  fn extract_calls(&self, code: &Calcit, current_ns: &str) -> Vec<(String, String)> {
    let mut calls = vec![];
    Self::extract_calls_recursive(code, current_ns, &mut calls);
    // Deduplicate while preserving order
    let mut seen = HashSet::new();
    calls.retain(|item| seen.insert(item.clone()));
    calls
  }

  fn extract_calls_recursive(code: &Calcit, current_ns: &str, calls: &mut Vec<(String, String)>) {
    match code {
      Calcit::Import(import) => {
        calls.push((import.ns.to_string(), import.def.to_string()));
      }
      Calcit::Symbol { sym, info, .. } => {
        // Check if this symbol refers to a definition in the same namespace
        let program_code = PROGRAM_CODE_DATA.read().ok();
        if let Some(ref code_data) = program_code {
          let mut found = false;

          // First check current namespace
          if let Some(file_data) = code_data.get(current_ns) {
            if file_data.defs.contains_key(sym.as_ref()) {
              calls.push((current_ns.to_string(), sym.to_string()));
              found = true;
            }
          }

          // Then check import map
          if !found {
            if let Some(file_data) = code_data.get(info.at_ns.as_ref()) {
              if let Some(import_rule) = file_data.import_map.get(sym.as_ref()) {
                match &**import_rule {
                  crate::program::ImportRule::NsReferDef(ns, def) => {
                    calls.push((ns.to_string(), def.to_string()));
                    found = true;
                  }
                  crate::program::ImportRule::NsAs(ns) => {
                    // For :as imports, we'd need more context to know the def
                    // This is typically handled via Calcit::Import
                    let _ = ns;
                  }
                  crate::program::ImportRule::NsDefault(ns) => {
                    calls.push((ns.to_string(), "default".to_string()));
                    found = true;
                  }
                }
              }
            }
          }

          // Finally, check if it's implicitly available from calcit.core
          // (unless we're already in calcit.core to avoid double-counting)
          if !found && current_ns != "calcit.core" {
            if let Some(core_data) = code_data.get("calcit.core") {
              if core_data.defs.contains_key(sym.as_ref()) {
                calls.push(("calcit.core".to_string(), sym.to_string()));
              }
            }
          }
        }
      }
      Calcit::List(list) => {
        list
          .traverse_result::<String>(&mut |item| {
            Self::extract_calls_recursive(item, current_ns, calls);
            Ok(())
          })
          .ok();
      }
      Calcit::Fn { info, .. } => {
        // Extract calls from function body
        for expr in info.body.iter() {
          Self::extract_calls_recursive(expr, current_ns, calls);
        }
      }
      Calcit::Macro { info, .. } => {
        // Extract calls from macro body
        for expr in info.body.iter() {
          Self::extract_calls_recursive(expr, current_ns, calls);
        }
      }
      Calcit::Thunk(thunk) => match thunk {
        crate::calcit::CalcitThunk::Code { code, .. } => {
          Self::extract_calls_recursive(code, current_ns, calls);
        }
        crate::calcit::CalcitThunk::Evaled { code, value } => {
          Self::extract_calls_recursive(code, current_ns, calls);
          Self::extract_calls_recursive(value, current_ns, calls);
        }
      },
      Calcit::Tuple(tuple) => {
        for item in &tuple.extra {
          Self::extract_calls_recursive(item, current_ns, calls);
        }
      }
      Calcit::Map(map) => {
        for (k, v) in map.iter() {
          Self::extract_calls_recursive(k, current_ns, calls);
          Self::extract_calls_recursive(v, current_ns, calls);
        }
      }
      Calcit::Set(set) => {
        for item in set.iter() {
          Self::extract_calls_recursive(item, current_ns, calls);
        }
      }
      // Other cases don't contain calls
      _ => {}
    }
  }

  fn get_source_type(&self, ns: &str) -> String {
    if self.is_core_ns(ns) {
      "core".to_string()
    } else if let Some(ref pkg) = self.config.package_name {
      if ns.starts_with(pkg) || ns.starts_with(&format!("{pkg}.")) {
        "project".to_string()
      } else {
        "external".to_string()
      }
    } else {
      "project".to_string()
    }
  }

  fn is_core_ns(&self, ns: &str) -> bool {
    ns.starts_with("calcit.") || ns == "calcit.core"
  }

  fn count_def_types(&self, _program_code: &ProgramCodeData) -> (usize, usize) {
    let mut project = 0;
    let mut core = 0;

    for fqn in &self.reachable {
      if let Some((ns, _)) = fqn.split_once('/') {
        if self.is_core_ns(ns) {
          core += 1;
        } else {
          project += 1;
        }
      }
    }

    (project, core)
  }

  fn find_unused_definitions(&self, program_code: &ProgramCodeData) -> Vec<UnusedDefinition> {
    let mut unused = vec![];

    for (ns, file_data) in program_code.iter() {
      // Skip core namespaces
      if self.is_core_ns(ns) {
        continue;
      }

      // Skip external packages if package name is set
      if let Some(ref pkg) = self.config.package_name {
        if !ns.starts_with(pkg) && !ns.starts_with(&format!("{pkg}.")) {
          continue;
        }
      }

      for (def, entry) in &file_data.defs {
        let fqn = format!("{ns}/{def}");
        if !self.reachable.contains(&fqn) {
          unused.push(UnusedDefinition {
            ns: ns.to_string(),
            def: def.to_string(),
            fqn,
            doc: if entry.doc.is_empty() { None } else { Some(entry.doc.to_string()) },
          });
        }
      }
    }

    // Sort by namespace then by definition name
    unused.sort_by(|a, b| match a.ns.cmp(&b.ns) {
      std::cmp::Ordering::Equal => a.def.cmp(&b.def),
      other => other,
    });

    unused
  }
}

/// Prune a call tree to only keep nodes whose namespace starts with `prefix`,
/// while preserving ancestors that lead to matching descendants.
fn prune_tree_by_ns_prefix(mut node: CallTreeNode, prefix: &str) -> CallTreeNode {
  // First prune children recursively
  let children = std::mem::take(&mut node.calls);
  let pruned_children: Vec<CallTreeNode> = children
    .into_iter()
    .map(|child| prune_tree_by_ns_prefix(child, prefix))
    .filter(|child| tree_contains_ns_prefix(child, prefix))
    .collect();

  // Replace children with pruned list
  node.calls = pruned_children;

  node
}

fn tree_contains_ns_prefix(node: &CallTreeNode, prefix: &str) -> bool {
  if node.ns.starts_with(prefix) {
    return true;
  }
  for c in &node.calls {
    if tree_contains_ns_prefix(c, prefix) {
      return true;
    }
  }
  false
}

/// Compute stats from a (possibly pruned) call tree.
fn compute_stats_from_tree<F>(root: &CallTreeNode, is_core_ns: F) -> (usize, usize, usize, usize, usize)
where
  F: Fn(&str) -> bool,
{
  use std::collections::HashSet;

  struct WalkState {
    visited: HashSet<String>,
    project: usize,
    core: usize,
    circular: usize,
    max_depth: usize,
  }

  fn walk<F>(n: &CallTreeNode, depth: usize, state: &mut WalkState, is_core_ns: &F)
  where
    F: Fn(&str) -> bool,
  {
    if depth > state.max_depth {
      state.max_depth = depth;
    }
    if n.circular {
      state.circular += 1;
    }
    if !state.visited.contains(&n.fqn) {
      state.visited.insert(n.fqn.clone());
      if is_core_ns(&n.ns) {
        state.core += 1;
      } else {
        state.project += 1;
      }
    }
    for c in &n.calls {
      walk(c, depth + 1, state, is_core_ns);
    }
  }

  let mut state = WalkState {
    visited: HashSet::new(),
    project: 0,
    core: 0,
    circular: 0,
    max_depth: 0,
  };

  walk(root, 0, &mut state, &is_core_ns);

  (state.visited.len(), state.project, state.core, state.circular, state.max_depth)
}

/// Format the call tree result for LLM consumption
pub fn format_for_llm(result: &CallTreeResult) -> String {
  let mut output = String::new();

  output.push_str("# Call Tree Analysis\n\n");
  output.push_str(&format!("**Entry Point:** `{}`\n\n", result.entry));

  output.push_str("## Statistics\n\n");
  output.push_str(&format!("- Reachable definitions: {}\n", result.stats.reachable_count));
  output.push_str(&format!("- Project definitions: {}\n", result.stats.project_defs));
  output.push_str(&format!("- Core/library definitions: {}\n", result.stats.core_defs));
  output.push_str(&format!("- Maximum call depth: {}\n", result.stats.max_depth));
  if result.stats.circular_count > 0 {
    output.push_str(&format!("- Circular references: {}\n", result.stats.circular_count));
  }
  output.push('\n');

  output.push_str("## Call Tree Structure\n\n");
  output.push_str("```\n");
  format_tree_node(&result.tree, &mut output, "", true);
  output.push_str("```\n\n");

  if let Some(ref unused) = result.unused_definitions {
    output.push_str("## Unused Definitions\n\n");
    if unused.is_empty() {
      output.push_str("No unused definitions found.\n");
    } else {
      output.push_str(&format!("Found {} unused definition(s):\n\n", unused.len()));
      for def in unused {
        output.push_str(&format!("- `{}`", def.fqn));
        if let Some(ref doc) = def.doc {
          output.push_str(&format!(" - {doc}"));
        }
        output.push('\n');
      }
    }
  }

  output
}

fn format_tree_node(node: &CallTreeNode, output: &mut String, prefix: &str, is_last: bool) {
  let connector = if is_last { "└── " } else { "├── " };
  let marker = if node.circular {
    " [CIRCULAR]"
  } else if node.seen {
    " [seen]"
  } else if node.source == "core" {
    " [core]"
  } else if node.source == "external" {
    " [ext]"
  } else {
    ""
  };

  output.push_str(&format!("{}{}{}{}\n", prefix, connector, node.fqn, marker));

  let child_prefix = format!("{}{}   ", prefix, if is_last { " " } else { "│" });

  for (i, child) in node.calls.iter().enumerate() {
    let is_last_child = i == node.calls.len() - 1;
    format_tree_node(child, output, &child_prefix, is_last_child);
  }
}

/// Format the call tree result as JSON for machine consumption
pub fn format_as_json(result: &CallTreeResult) -> Result<String, String> {
  serde_json::to_string_pretty(result).map_err(|e| format!("Failed to serialize to JSON: {e}"))
}

/// Main entry point for call tree analysis
pub fn analyze_call_graph(
  entry_ns: &str,
  entry_def: &str,
  include_core: bool,
  max_depth: usize,
  show_unused: bool,
  package_name: Option<String>,
  ns_prefix: Option<String>,
) -> Result<CallTreeResult, String> {
  let config = CallTreeConfig {
    include_core,
    max_depth,
    show_unused,
    package_name,
    ns_prefix,
  };

  let mut analyzer = CallTreeAnalyzer::new(config);
  analyzer.analyze(entry_ns, entry_def)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Call count analysis
// ═══════════════════════════════════════════════════════════════════════════════

/// Result of call count analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallCountResult {
  /// The entry point of the analysis
  pub entry: String,
  /// Map of definition FQN to call count
  pub counts: Vec<CallCountEntry>,
  /// Total unique definitions called
  pub total_definitions: usize,
  /// Total call count
  pub total_calls: usize,
}

/// A single entry in the call count result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallCountEntry {
  /// Full qualified name (ns/def)
  pub fqn: String,
  /// Namespace
  pub ns: String,
  /// Definition name
  pub def: String,
  /// Number of times this definition is called
  pub count: usize,
  /// Source type: "project", "core", or "external"
  pub source: String,
  /// Whether this definition has documentation
  pub has_doc: bool,
  /// Number of examples
  pub examples_count: usize,
}

/// Call count analyzer
struct CallCountAnalyzer {
  /// Whether to include core/library calls
  include_core: bool,
  /// Namespace prefix filter
  ns_prefix: Option<String>,
  /// Track visited definitions to avoid infinite loops
  visited: HashSet<String>,
  /// Count of calls for each definition
  call_counts: HashMap<String, usize>,
}

impl CallCountAnalyzer {
  fn new(include_core: bool, ns_prefix: Option<String>) -> Self {
    CallCountAnalyzer {
      include_core,
      ns_prefix,
      visited: HashSet::new(),
      call_counts: HashMap::new(),
    }
  }

  fn analyze(&mut self, entry_ns: &str, entry_def: &str) -> Result<CallCountResult, String> {
    let fqn = format!("{entry_ns}/{entry_def}");

    // Count the entry itself if it matches filters
    let entry_is_core = self.is_core_ns(entry_ns);
    let entry_matches_core = self.include_core || !entry_is_core;
    let entry_matches_prefix = self.ns_prefix.as_ref().map(|p| entry_ns.starts_with(p)).unwrap_or(true);
    if entry_matches_core && entry_matches_prefix {
      *self.call_counts.entry(fqn.clone()).or_insert(0) += 1;
    }

    // Traverse and count
    self.traverse(entry_ns, entry_def)?;

    // Build result
    let program_code = PROGRAM_CODE_DATA.read().map_err(|e| format!("Failed to read program code: {e}"))?;

    let mut counts: Vec<CallCountEntry> = self
      .call_counts
      .iter()
      .map(|(fqn, &count)| {
        let (ns, def) = fqn.split_once('/').unwrap_or(("", fqn));

        // Get doc and examples info
        let (has_doc, examples_count) = program_code
          .get(ns)
          .and_then(|file_data| file_data.defs.get(def))
          .map(|code_entry| {
            let has_doc = !code_entry.doc.is_empty();
            let examples_count = code_entry.examples.len();
            (has_doc, examples_count)
          })
          .unwrap_or((false, 0));

        CallCountEntry {
          fqn: fqn.clone(),
          ns: ns.to_string(),
          def: def.to_string(),
          count,
          source: self.get_source_type(ns),
          has_doc,
          examples_count,
        }
      })
      .collect();

    // Sort by count descending by default
    counts.sort_by(|a, b| b.count.cmp(&a.count));

    let total_calls: usize = counts.iter().map(|e| e.count).sum();

    Ok(CallCountResult {
      entry: format!("{entry_ns}/{entry_def}"),
      total_definitions: counts.len(),
      total_calls,
      counts,
    })
  }

  fn traverse(&mut self, ns: &str, def: &str) -> Result<(), String> {
    let fqn = format!("{ns}/{def}");

    // Already visited, skip to avoid infinite loop
    if self.visited.contains(&fqn) {
      return Ok(());
    }
    self.visited.insert(fqn.clone());

    let program_code = PROGRAM_CODE_DATA.read().map_err(|e| format!("Failed to read program code: {e}"))?;

    // Get the definition
    let code_entry = match program_code.get(ns) {
      Some(file_data) => file_data.defs.get(def).map(|e| &e.code),
      None => None,
    };

    // Extract calls from the code
    if let Some(code) = code_entry {
      let call_refs = self.extract_calls(code, ns);

      // Release the lock before recursive calls
      drop(program_code);

      for (call_ns, call_def) in call_refs {
        let is_core = self.is_core_ns(&call_ns);
        let matches_core = self.include_core || !is_core;
        let matches_prefix = self.ns_prefix.as_ref().map(|p| call_ns.starts_with(p)).unwrap_or(true);

        // Increment count only when it matches filters
        if matches_core && matches_prefix {
          let call_fqn = format!("{call_ns}/{call_def}");
          *self.call_counts.entry(call_fqn).or_insert(0) += 1;
        }

        // Always traverse to discover deeper matching calls
        self.traverse(&call_ns, &call_def)?;
      }
    }

    Ok(())
  }

  fn extract_calls(&self, code: &Calcit, current_ns: &str) -> Vec<(String, String)> {
    let mut calls = vec![];
    CallTreeAnalyzer::extract_calls_recursive(code, current_ns, &mut calls);
    calls
  }

  fn is_core_ns(&self, ns: &str) -> bool {
    ns.starts_with("calcit.") || ns == "calcit.core"
  }

  fn get_source_type(&self, ns: &str) -> String {
    if self.is_core_ns(ns) {
      "core".to_string()
    } else {
      "project".to_string()
    }
  }
}

/// Count calls from entry point
pub fn count_calls(entry_ns: &str, entry_def: &str, include_core: bool, ns_prefix: Option<String>) -> Result<CallCountResult, String> {
  let mut analyzer = CallCountAnalyzer::new(include_core, ns_prefix);
  analyzer.analyze(entry_ns, entry_def)
}

/// Format call count result for display
pub fn format_count_for_display(result: &CallCountResult, sort: &str) -> String {
  let mut output = String::new();

  output.push_str("# Call Count Analysis\n\n");
  output.push_str(&format!("**Entry Point:** `{}`\n\n", result.entry));
  output.push_str(&format!("**Total Definitions:** {}\n", result.total_definitions));
  output.push_str(&format!("**Total Calls:** {}\n\n", result.total_calls));

  output.push_str("## Call Counts\n\n");
  output.push_str("| Count | Definition                               | Doc | Examples |\n");
  output.push_str("|------:|:-----------------------------------------|:---:|:--------:|\n");

  let mut counts = result.counts.clone();
  match sort {
    "name" => counts.sort_by(|a, b| a.fqn.cmp(&b.fqn)),
    _ => counts.sort_by(|a, b| b.count.cmp(&a.count)),
  }

  for entry in &counts {
    let doc_mark = if entry.has_doc { "✓" } else { " " };
    let examples_str = if entry.examples_count > 0 {
      format!("{:>8}", entry.examples_count)
    } else {
      format!("{:>8}", "")
    };

    // Format count with at least 2 characters width (right-aligned)
    let count_str = format!("{:>5}", entry.count);

    // Format definition with at least 20 characters width (left-aligned)
    let def_str = format!("{:<40}", entry.fqn);

    output.push_str(&format!("| {count_str} | {def_str} | {doc_mark}   | {examples_str} |\n"));
  }

  output
}

/// Format call count result as JSON
pub fn format_count_as_json(result: &CallCountResult) -> Result<String, String> {
  serde_json::to_string_pretty(result).map_err(|e| format!("Failed to serialize to JSON: {e}"))
}
