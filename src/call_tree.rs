//! Call tree analysis module for Calcit programs
//!
//! This module extracts and analyzes the call tree structure of a Calcit program,
//! starting from a specified entry point. It helps understand code dependencies
//! and identify unused definitions.

use crate::calcit::Calcit;
use crate::program::{PROGRAM_CODE_DATA, ProgramCodeData};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
  /// Track definitions that have been fully expanded (shown with children)
  expanded: HashSet<String>,
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
      expanded: HashSet::new(),
      reachable: HashSet::new(),
      circular_count: 0,
      max_depth: 0,
    }
  }

  /// Analyze the call tree starting from the given entry point
  pub fn analyze(&mut self, entry_ns: &str, entry_def: &str) -> Result<CallTreeResult, String> {
    let fqn = format!("{entry_ns}/{entry_def}");

    // Build the call tree
    let tree = self.build_tree(entry_ns, entry_def, 0)?;

    // Calculate statistics
    let program_code = PROGRAM_CODE_DATA.read().map_err(|e| format!("Failed to read program code: {e}"))?;

    let (project_defs, core_defs) = self.count_def_types(&program_code);

    let stats = CallTreeStats {
      reachable_count: self.reachable.len(),
      circular_count: self.circular_count,
      project_defs,
      core_defs,
      max_depth: self.max_depth,
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
    if self.expanded.contains(&fqn) {
      return Ok(CallTreeNode {
        ns: ns.to_string(),
        def: def.to_string(),
        fqn: fqn.clone(),
        doc: None,
        calls: vec![],
        circular: false,
        seen: true,
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

        // Filter by namespace prefix if specified
        if let Some(ref prefix) = self.config.ns_prefix {
          if !call_ns.starts_with(prefix) {
            continue;
          }
        }

        let child_tree = self.build_tree(&call_ns, &call_def, depth + 1)?;
        calls.push(child_tree);
      }
    } else {
      drop(program_code);
    }

    // Unmark from current path (allow revisiting in different branches)
    self.visited.remove(&fqn);

    // Mark as expanded so subsequent occurrences show as "seen"
    self.expanded.insert(fqn.clone());

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
          if let Some(file_data) = code_data.get(current_ns) {
            if file_data.defs.contains_key(sym.as_ref()) {
              calls.push((current_ns.to_string(), sym.to_string()));
            }
          }
          // Also check if it's imported via the import map
          if let Some(file_data) = code_data.get(info.at_ns.as_ref()) {
            if let Some(import_rule) = file_data.import_map.get(sym.as_ref()) {
              match &**import_rule {
                crate::program::ImportRule::NsReferDef(ns, def) => {
                  calls.push((ns.to_string(), def.to_string()));
                }
                crate::program::ImportRule::NsAs(ns) => {
                  // For :as imports, we'd need more context to know the def
                  // This is typically handled via Calcit::Import
                  let _ = ns;
                }
                crate::program::ImportRule::NsDefault(ns) => {
                  calls.push((ns.to_string(), "default".to_string()));
                }
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
pub fn analyze_call_tree(
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
