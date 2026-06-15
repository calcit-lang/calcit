//! Effects graph analysis: State / Transform / Effect decomposition per definition.

use crate::builtins;
use crate::calcit::{Calcit, CalcitFnArgs, CalcitLocal, CalcitProc, CalcitSyntax};
use crate::call_tree::{analyze_call_graph, CallTreeNode};
use crate::program::{ImportRule, PROGRAM_CODE_DATA};
use cirru_edn::EdnTag;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Configuration for effects graph analysis.
#[derive(Debug, Clone, Default)]
pub struct EffectsGraphConfig {
  pub include_core: bool,
  pub max_depth: usize,
  pub ns_prefix: Option<String>,
  pub detail: EffectsGraphDetail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EffectsGraphDetail {
  #[default]
  Summary,
  Full,
  Minimal,
}

/// A detected state item inside a definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateItem {
  pub kind: String,
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub type_hint: Option<String>,
}

/// A classified effect occurrence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EffectItem {
  pub kind: String,
  pub target: String,
  pub count: usize,
}

/// Compressed transform summary for a definition.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransformInfo {
  pub summary: String,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub control: Vec<String>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub calls: Vec<String>,
}

/// Per-definition decomposition node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectsGraphNode {
  pub ns: String,
  pub def: String,
  pub fqn: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub doc: Option<String>,
  pub source: String,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub state: Vec<StateItem>,
  pub transform: TransformInfo,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub effects: Vec<EffectItem>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub children: Vec<EffectsGraphNode>,
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  pub circular: bool,
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  pub seen: bool,
}

/// Analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectsGraphResult {
  pub entry: String,
  pub tree: EffectsGraphNode,
  pub stats: EffectsGraphStats,
  pub display: EffectsGraphDisplayMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectsGraphDisplayMeta {
  /// configured `--max-depth` (0 = unlimited)
  pub max_depth_limit: usize,
  pub detail: String,
  pub include_core: bool,
  pub ns_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectsGraphStats {
  pub reachable_count: usize,
  pub effect_sites: usize,
  pub state_items: usize,
  pub max_depth: usize,
  pub subgraph_count: usize,
}

struct TagIndex {
  core: HashMap<String, HashSet<EdnTag>>,
}

impl TagIndex {
  fn load() -> Result<Self, String> {
    let snapshot = crate::load_core_snapshot()?;
    let mut core = HashMap::new();
    if let Some(file) = snapshot.files.get("calcit.core") {
      for (def, entry) in &file.defs {
        core.insert(def.clone(), entry.tags.clone());
      }
    }
    Ok(TagIndex { core })
  }

  fn tags_for(&self, name: &str) -> Option<&HashSet<EdnTag>> {
    self.core.get(name)
  }
}

struct DefAnalysis {
  state: Vec<StateItem>,
  effects: HashMap<String, EffectItem>,
  transform: TransformInfo,
}

pub struct EffectsGraphAnalyzer {
  config: EffectsGraphConfig,
  tag_index: TagIndex,
  visited: HashSet<String>,
  expanded: HashMap<String, bool>,
  reachable: HashSet<String>,
  max_depth: usize,
  total_effect_sites: usize,
  total_state_items: usize,
}

impl EffectsGraphAnalyzer {
  pub fn new(config: EffectsGraphConfig) -> Result<Self, String> {
    Ok(EffectsGraphAnalyzer {
      config,
      tag_index: TagIndex::load()?,
      visited: HashSet::new(),
      expanded: HashMap::new(),
      reachable: HashSet::new(),
      max_depth: 0,
      total_effect_sites: 0,
      total_state_items: 0,
    })
  }

  pub fn analyze(&mut self, entry_ns: &str, entry_def: &str) -> Result<EffectsGraphResult, String> {
    let entry = format!("{entry_ns}/{entry_def}");
    let mut tree = self.build_node(entry_ns, entry_def, 0)?;
    if let Some(ref prefix) = self.config.ns_prefix {
      tree = prune_tree_by_ns_prefix(tree, prefix);
    }

    let subgraph_count = count_subgraph_nodes(&tree);

    let stats = EffectsGraphStats {
      reachable_count: self.reachable.len(),
      effect_sites: self.total_effect_sites,
      state_items: self.total_state_items,
      max_depth: self.max_depth,
      subgraph_count,
    };

    let display = EffectsGraphDisplayMeta {
      max_depth_limit: self.config.max_depth,
      detail: match self.config.detail {
        EffectsGraphDetail::Full => "full".into(),
        EffectsGraphDetail::Minimal => "minimal".into(),
        EffectsGraphDetail::Summary => "summary".into(),
      },
      include_core: self.config.include_core,
      ns_prefix: self.config.ns_prefix.clone(),
    };

    Ok(EffectsGraphResult {
      entry,
      tree,
      stats,
      display,
    })
  }

  fn build_node(&mut self, ns: &str, def: &str, depth: usize) -> Result<EffectsGraphNode, String> {
    let fqn = format!("{ns}/{def}");
    if depth > self.max_depth {
      self.max_depth = depth;
    }

    if self.config.max_depth > 0 && depth >= self.config.max_depth {
      return Ok(empty_shell_node(ns, def, &fqn, self.source_type(ns)));
    }

    if self.visited.contains(&fqn) {
      return Ok(EffectsGraphNode {
        ns: ns.to_string(),
        def: def.to_string(),
        fqn,
        doc: None,
        source: self.source_type(ns),
        state: vec![],
        transform: TransformInfo::default(),
        effects: vec![],
        children: vec![],
        circular: true,
        seen: false,
      });
    }

    if let Some(&had_children) = self.expanded.get(&fqn) {
      return Ok(EffectsGraphNode {
        ns: ns.to_string(),
        def: def.to_string(),
        fqn,
        doc: None,
        source: self.source_type(ns),
        state: vec![],
        transform: TransformInfo::default(),
        effects: vec![],
        children: vec![],
        circular: false,
        seen: had_children,
      });
    }

    self.visited.insert(fqn.clone());
    self.reachable.insert(fqn.clone());

    let program_code = PROGRAM_CODE_DATA.read().map_err(|e| format!("Failed to read program code: {e}"))?;
    let (code, doc, schema_return) = match program_code.get(ns) {
      Some(file) => match file.defs.get(def) {
        Some(entry) => {
          let doc = if entry.doc.is_empty() { None } else { Some(entry.doc.to_string()) };
          let return_hint = format_type_hint(&entry.schema);
          (Some(entry.code.clone()), doc, return_hint)
        }
        None => (None, None, None),
      },
      None => (None, None, None),
    };

    let mut children = vec![];
    let mut analysis = DefAnalysis {
      state: vec![],
      effects: HashMap::new(),
      transform: TransformInfo::default(),
    };

    if let Some(code) = code {
      self.analyze_code(&code, ns, doc.as_deref(), &mut analysis);
      if let Some(hint) = schema_return {
        analysis.state.push(StateItem {
          kind: "return".into(),
          name: "return".into(),
          type_hint: Some(hint),
        });
      }

      let call_refs = extract_call_targets(&code, ns);
      drop(program_code);

      for (call_ns, call_def) in call_refs {
        if !self.config.include_core && self.is_core_ns(&call_ns) {
          continue;
        }
        if call_ns == ns && call_def == def {
          continue;
        }
        children.push(self.build_node(&call_ns, &call_def, depth + 1)?);
      }
    } else {
      drop(program_code);
    }

    self.visited.remove(&fqn);
    self.expanded.insert(fqn.clone(), !children.is_empty());

    self.total_effect_sites += analysis.effects.values().map(|e| e.count).sum::<usize>();
    self.total_state_items += analysis.state.len();

    let effects: Vec<EffectItem> = {
      let mut items: Vec<EffectItem> = analysis.effects.into_values().collect();
      items.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.target.cmp(&b.target)));
      items
    };

    Ok(EffectsGraphNode {
      ns: ns.to_string(),
      def: def.to_string(),
      fqn,
      doc,
      source: self.source_type(ns),
      state: analysis.state,
      transform: analysis.transform,
      effects,
      children,
      circular: false,
      seen: false,
    })
  }

  fn analyze_code(&self, code: &Calcit, current_ns: &str, doc: Option<&str>, out: &mut DefAnalysis) {
    match code {
      Calcit::Fn { info, .. } => {
        extract_fn_params(&info.args, &info.arg_types, &mut out.state);
        for expr in info.body.iter() {
          self.walk_expr(expr, current_ns, out, 0);
        }
        if out.transform.summary.is_empty() {
          out.transform.summary = doc_or_generated_summary(doc, &out.transform.control, &out.transform.calls);
        }
      }
      Calcit::Macro { info, .. } => {
        for expr in info.body.iter() {
          self.walk_expr(expr, current_ns, out, 0);
        }
        if out.transform.summary.is_empty() {
          out.transform.summary = doc_or_generated_summary(doc, &out.transform.control, &out.transform.calls);
        }
      }
      _ => {
        self.walk_expr(code, current_ns, out, 0);
        if out.transform.summary.is_empty() {
          out.transform.summary = doc_or_generated_summary(doc, &out.transform.control, &out.transform.calls);
        }
      }
    }
  }

  fn walk_expr(&self, code: &Calcit, current_ns: &str, out: &mut DefAnalysis, depth: usize) {
    if let Calcit::List(list) = code {
      if let Some(head) = list.first() {
        self.inspect_call_head(head, current_ns, out);
        if depth < 3 {
          record_control_head(head, &mut out.transform.control);
        }
      }
      let _ = list.traverse_result::<String>(&mut |item| {
        self.walk_expr(item, current_ns, out, depth + 1);
        Ok(())
      });
      return;
    }

    match code {
      Calcit::Fn { info, .. } => {
        for expr in info.body.iter() {
          self.walk_expr(expr, current_ns, out, depth);
        }
      }
      Calcit::Macro { info, .. } => {
        for expr in info.body.iter() {
          self.walk_expr(expr, current_ns, out, depth);
        }
      }
      Calcit::Thunk(crate::calcit::CalcitThunk::Code { code, .. }) => {
        self.walk_expr(code, current_ns, out, depth);
      }
      Calcit::Tuple(tuple) => {
        for item in &tuple.extra {
          self.walk_expr(item, current_ns, out, depth);
        }
      }
      Calcit::Map(map) => {
        for (k, v) in map.iter() {
          self.walk_expr(k, current_ns, out, depth);
          self.walk_expr(v, current_ns, out, depth);
        }
      }
      Calcit::Set(set) => {
        for item in set.iter() {
          self.walk_expr(item, current_ns, out, depth);
        }
      }
      _ => {}
    }
  }

  fn inspect_call_head(&self, head: &Calcit, current_ns: &str, out: &mut DefAnalysis) {
    let Some((name, ns_hint)) = call_operator(head, current_ns) else {
      return;
    };

    if is_state_operator(&name) {
      record_state_operator(&name, &mut out.state);
      if matches!(name.as_str(), "defatom" | "atom" | "reset!" | "swap!" | "deref") {
        return;
      }
    }

    let tags = ns_hint
      .as_deref()
      .filter(|ns| *ns == "calcit.core")
      .and_then(|_| self.tag_index.tags_for(&name))
      .or_else(|| self.tag_index.tags_for(&name));

    for kind in classify_call(&name, tags) {
      let key = format!("{kind}::{name}");
      out.effects.entry(key).and_modify(|item| item.count += 1).or_insert(EffectItem {
        kind,
        target: name.clone(),
        count: 1,
      });
    }

    if let Some((call_ns, call_def)) = resolve_def_call(head, current_ns) {
      if !is_meaningful_call_target(&call_ns, &call_def, self.config.include_core) {
        return;
      }
      let target = format!("{call_ns}/{call_def}");
      if !out.transform.calls.contains(&target) {
        out.transform.calls.push(target);
      }
    }
  }

  fn source_type(&self, ns: &str) -> String {
    if self.is_core_ns(ns) {
      "core".into()
    } else if ns.starts_with("js/") {
      "external".into()
    } else {
      "project".into()
    }
  }

  fn is_core_ns(&self, ns: &str) -> bool {
    ns == "calcit.core" || ns.starts_with("calcit.")
  }
}

fn empty_shell_node(ns: &str, def: &str, fqn: &str, source: String) -> EffectsGraphNode {
  EffectsGraphNode {
    ns: ns.to_string(),
    def: def.to_string(),
    fqn: fqn.to_string(),
    doc: None,
    source,
    state: vec![],
    transform: TransformInfo::default(),
    effects: vec![],
    children: vec![],
    circular: false,
    seen: false,
  }
}

fn extract_fn_params(
  args: &CalcitFnArgs,
  arg_types: &[std::sync::Arc<crate::calcit::CalcitTypeAnnotation>],
  state: &mut Vec<StateItem>,
) {
  match args {
    CalcitFnArgs::Args(indices) => {
      for (idx, local_idx) in indices.iter().enumerate() {
        state.push(StateItem {
          kind: "param".into(),
          name: CalcitLocal::read_name(*local_idx).to_string(),
          type_hint: arg_types.get(idx).map(|t| t.to_string()),
        });
      }
    }
    CalcitFnArgs::MarkedArgs(labels) => {
      let mut arg_idx = 0usize;
      for label in labels {
        if let crate::calcit::CalcitArgLabel::Idx(local_idx) = label {
          state.push(StateItem {
            kind: "param".into(),
            name: label.to_string(),
            type_hint: arg_types.get(arg_idx).map(|t| t.to_string()),
          });
          arg_idx += 1;
          let _ = local_idx;
        }
      }
    }
  }
}

fn call_operator(head: &Calcit, current_ns: &str) -> Option<(String, Option<String>)> {
  match head {
    Calcit::Proc(proc) => Some((proc.to_string(), Some("calcit.core".into()))),
    Calcit::Syntax(syntax, _) => Some((syntax.to_string(), Some("calcit.core".into()))),
    Calcit::Registered(name) => Some((name.to_string(), None)),
    Calcit::Import(import) => Some((import.def.to_string(), Some(import.ns.to_string()))),
    Calcit::Symbol { sym, info, .. } => {
      if builtins::is_proc_name(sym) {
        return Some((sym.to_string(), None));
      }
      let program_code = PROGRAM_CODE_DATA.read().ok()?;
      if let Some(file) = program_code.get(current_ns) {
        if file.defs.contains_key(sym.as_ref()) {
          return Some((sym.to_string(), Some(current_ns.into())));
        }
        if let Some(rule) = file.import_map.get(sym.as_ref()) {
          match &**rule {
            ImportRule::NsReferDef(ns, def) => return Some((def.to_string(), Some(ns.to_string()))),
            ImportRule::NsDefault(ns) => return Some(("default".into(), Some(ns.to_string()))),
            ImportRule::NsAs(_) => {}
          }
        }
      }
      if current_ns != "calcit.core" {
        if let Some(core) = program_code.get("calcit.core") {
          if core.defs.contains_key(sym.as_ref()) {
            return Some((sym.to_string(), Some("calcit.core".into())));
          }
        }
      }
      Some((sym.to_string(), Some(info.at_ns.to_string())))
    }
    _ => None,
  }
}

fn resolve_def_call(head: &Calcit, current_ns: &str) -> Option<(String, String)> {
  match head {
    Calcit::Import(import) => Some((import.ns.to_string(), import.def.to_string())),
    Calcit::Symbol { sym, info, .. } => {
      let program_code = PROGRAM_CODE_DATA.read().ok()?;
      if let Some(file) = program_code.get(current_ns) {
        if file.defs.contains_key(sym.as_ref()) {
          return Some((current_ns.to_string(), sym.to_string()));
        }
        if let Some(rule) = file.import_map.get(sym.as_ref()) {
          match &**rule {
            ImportRule::NsReferDef(ns, def) => return Some((ns.to_string(), def.to_string())),
            ImportRule::NsDefault(ns) => return Some((ns.to_string(), "default".into())),
            ImportRule::NsAs(_) => {}
          }
        }
      }
      if current_ns != "calcit.core" {
        if let Some(core) = program_code.get("calcit.core") {
          if core.defs.contains_key(sym.as_ref()) {
            return Some(("calcit.core".into(), sym.to_string()));
          }
        }
      }
      let _ = info;
      None
    }
    Calcit::Fn { info, .. } => info
      .def_ref
      .as_ref()
      .map(|def_ref| (def_ref.def_ns.to_string(), def_ref.def_name.to_string())),
    _ => None,
  }
}

fn extract_call_targets(code: &Calcit, current_ns: &str) -> Vec<(String, String)> {
  let mut calls = vec![];
  extract_call_targets_recursive(code, current_ns, &mut calls);
  let mut seen = HashSet::new();
  calls.retain(|item| seen.insert(item.clone()));
  calls
}

fn extract_call_targets_recursive(code: &Calcit, current_ns: &str, calls: &mut Vec<(String, String)>) {
  if let Some(pair) = resolve_def_call_from_expr(code, current_ns) {
    calls.push(pair);
  }
  match code {
    Calcit::List(list) => {
      let _ = list.traverse_result::<String>(&mut |item| {
        extract_call_targets_recursive(item, current_ns, calls);
        Ok(())
      });
    }
    Calcit::Fn { info, .. } => {
      for expr in info.body.iter() {
        extract_call_targets_recursive(expr, current_ns, calls);
      }
    }
    Calcit::Macro { info, .. } => {
      for expr in info.body.iter() {
        extract_call_targets_recursive(expr, current_ns, calls);
      }
    }
    Calcit::Thunk(crate::calcit::CalcitThunk::Code { code, .. }) => {
      extract_call_targets_recursive(code, current_ns, calls);
    }
    Calcit::Tuple(tuple) => {
      for item in &tuple.extra {
        extract_call_targets_recursive(item, current_ns, calls);
      }
    }
    Calcit::Map(map) => {
      for (k, v) in map.iter() {
        extract_call_targets_recursive(k, current_ns, calls);
        extract_call_targets_recursive(v, current_ns, calls);
      }
    }
    Calcit::Set(set) => {
      for item in set.iter() {
        extract_call_targets_recursive(item, current_ns, calls);
      }
    }
    _ => {}
  }
}

fn resolve_def_call_from_expr(code: &Calcit, current_ns: &str) -> Option<(String, String)> {
  match code {
    Calcit::Import(import) => Some((import.ns.to_string(), import.def.to_string())),
    Calcit::List(list) => list.first().and_then(|head| resolve_def_call(head, current_ns)),
    _ => None,
  }
}

fn is_state_operator(name: &str) -> bool {
  matches!(
    name,
    "defatom" | "reset!" | "swap!" | "atom" | "deref" | "add-watch" | "remove-watch"
  )
}

fn record_state_operator(name: &str, state: &mut Vec<StateItem>) {
  let kind = match name {
    "defatom" | "atom" => "atom",
    "reset!" | "swap!" => "atom-write",
    "add-watch" | "remove-watch" => "watch",
    "deref" => "atom-read",
    _ => "state",
  };
  state.push(StateItem {
    kind: kind.into(),
    name: name.into(),
    type_hint: None,
  });
}

fn record_control_head(head: &Calcit, control: &mut Vec<String>) {
  let label = match head {
    Calcit::Syntax(CalcitSyntax::If, _) => Some("if"),
    Calcit::Syntax(CalcitSyntax::CoreLet, _) => Some("let"),
    Calcit::Syntax(CalcitSyntax::Match, _) => Some("match"),
    Calcit::Syntax(CalcitSyntax::Try, _) => Some("try"),
    Calcit::Proc(CalcitProc::Foldl) => Some("foldl"),
    Calcit::Symbol { sym, .. } if sym.as_ref() == "foldl" || sym.as_ref() == "map" || sym.as_ref() == "filter" => Some(sym.as_ref()),
    _ => None,
  };
  if let Some(name) = label {
    control.push(name.to_string());
  }
}

pub fn classify_call(name: &str, tags: Option<&HashSet<EdnTag>>) -> Vec<String> {
  if let Some(kinds) = classify_by_name(name) {
    return kinds;
  }

  if let Some(descriptor) = builtins::registered_proc_descriptor(name) {
    return tags_to_effect_kinds(&descriptor.tags);
  }

  if let Some(tag_set) = tags {
    let kinds = tags_to_effect_kinds(tag_set);
    if !kinds.is_empty() {
      return kinds;
    }
  }

  heuristic_effect_kinds(name)
}

fn classify_by_name(name: &str) -> Option<Vec<String>> {
  let kinds = match name {
    "read-file" => vec!["io/read"],
    "write-file" => vec!["io/write"],
    "get-env" => vec!["env"],
    "raise" => vec!["control/raise"],
    "quit!" => vec!["control/quit"],
    "add-watch" | "remove-watch" => vec!["state/watch"],
    "eval" => vec!["interop/eval"],
    "hint-fn" => vec!["async"],
    "println" | "eprintln" | "echo" => vec!["console"],
    "render!" => vec!["render"],
    "generate-id!" | "cpu-time" | "&get-os" | "async-sleep" => vec!["io"],
    "try" => vec!["control"],
    "&doseq" => vec!["effect/sequential"],
    _ => return None,
  };
  Some(kinds.into_iter().map(str::to_string).collect())
}

fn tags_to_effect_kinds(tags: &HashSet<EdnTag>) -> Vec<String> {
  let mut kinds = vec![];
  if tags.contains(&EdnTag::new("log")) {
    kinds.push("console".into());
  }
  if tags.contains(&EdnTag::new("file")) {
    kinds.push("io/file".into());
  }
  if tags.contains(&EdnTag::new("env")) {
    kinds.push("env".into());
  }
  if tags.contains(&EdnTag::new("control")) {
    kinds.push("control".into());
  }
  if tags.contains(&EdnTag::new("interop")) {
    kinds.push("interop/host".into());
  }
  if tags.contains(&EdnTag::new("async")) {
    kinds.push("async".into());
  }
  if tags.contains(&EdnTag::new("watch")) {
    kinds.push("state/watch".into());
  }
  if tags.contains(&EdnTag::new("io")) && kinds.is_empty() {
    kinds.push("io".into());
  }
  if tags.contains(&EdnTag::new("effect")) {
    kinds.push("effect".into());
  }
  kinds.sort();
  kinds.dedup();
  kinds
}

fn heuristic_effect_kinds(name: &str) -> Vec<String> {
  if name.starts_with("js/") {
    return vec!["interop/js".into()];
  }
  if name.ends_with('!') && !matches!(name, "main!" | "reload!" | "quit!" | "reset!" | "swap!") {
    return vec!["unknown/effect!".into()];
  }
  vec![]
}

fn format_type_hint(schema: &std::sync::Arc<crate::calcit::CalcitTypeAnnotation>) -> Option<String> {
  use crate::calcit::CalcitTypeAnnotation;
  match schema.as_ref() {
    CalcitTypeAnnotation::Fn(fn_type) => Some(fn_type.return_type.to_string()),
    CalcitTypeAnnotation::Dynamic => None,
    other => Some(other.to_string()),
  }
}

fn doc_or_generated_summary(doc: Option<&str>, control: &[String], calls: &[String]) -> String {
  if let Some(text) = doc {
    let trimmed = text.lines().next().unwrap_or(text).trim();
    if !trimmed.is_empty() {
      return trimmed.to_string();
    }
  }
  let mut parts = vec![];
  if !control.is_empty() {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for item in control {
      *counts.entry(item.as_str()).or_default() += 1;
    }
    let control_summary: Vec<String> = counts
      .into_iter()
      .map(|(name, count)| if count > 1 { format!("{name}×{count}") } else { name.to_string() })
      .collect();
    parts.push(format!("control: {}", control_summary.join(", ")));
  }
  if !calls.is_empty() {
    parts.push(format!("calls: {}", calls.len()));
  }
  if parts.is_empty() { "transform".into() } else { parts.join("; ") }
}

fn prune_tree_by_ns_prefix(mut node: EffectsGraphNode, prefix: &str) -> EffectsGraphNode {
  let children = std::mem::take(&mut node.children);
  node.children = children
    .into_iter()
    .map(|child| prune_tree_by_ns_prefix(child, prefix))
    .filter(|child| tree_contains_ns_prefix(child, prefix))
    .collect();
  node
}

fn tree_contains_ns_prefix(node: &EffectsGraphNode, prefix: &str) -> bool {
  if node.ns.starts_with(prefix) {
    return true;
  }
  node.children.iter().any(|child| tree_contains_ns_prefix(child, prefix))
}

pub fn analyze_effects_graph(
  entry_ns: &str,
  entry_def: &str,
  include_core: bool,
  max_depth: usize,
  ns_prefix: Option<String>,
  detail: EffectsGraphDetail,
) -> Result<EffectsGraphResult, String> {
  let config = EffectsGraphConfig {
    include_core,
    max_depth,
    ns_prefix,
    detail,
  };
  let mut analyzer = EffectsGraphAnalyzer::new(config)?;
  analyzer.analyze(entry_ns, entry_def)
}

pub fn format_for_llm(result: &EffectsGraphResult) -> String {
  let mut output = String::new();
  output.push_str(&format!("# Birdview: `{}`\n\n", result.entry));
  output.push_str(&format_as_mermaid(result));
  output.push('\n');
  output.push_str(&format_birdview_legend(&result.tree));
  output
}

/// Mermaid birdview: State types, Transform chain, Effect kinds.
pub fn format_as_mermaid(result: &EffectsGraphResult) -> String {
  let model = build_birdview_model(&result.tree);
  let mut output = String::new();
  output.push_str("```mermaid\n");
  output.push_str(&render_mermaid_diagram(&model, &result.entry));
  output.push_str("```\n");
  output
}

#[derive(Debug, Clone)]
struct BirdviewTransform {
  id: String,
  label: String,
  collapsed: bool,
}

#[derive(Debug, Clone)]
struct BirdviewState {
  id: String,
  label: String,
}

#[derive(Debug, Clone)]
struct BirdviewEffect {
  id: String,
  kind: String,
}

#[derive(Debug, Clone)]
enum BirdviewEdge {
  Calls { from: String, to: String, collapsed: bool },
  HoldsState { transform: String, state: String },
  Triggers { transform: String, effect: String },
}

#[derive(Debug, Default)]
struct BirdviewModel {
  transforms: Vec<BirdviewTransform>,
  states: Vec<BirdviewState>,
  effects: Vec<BirdviewEffect>,
  edges: Vec<BirdviewEdge>,
}

fn build_birdview_model(root: &EffectsGraphNode) -> BirdviewModel {
  let mut model = BirdviewModel::default();
  let mut transform_index: HashMap<String, String> = HashMap::new();
  collect_birdview_transforms(root, &mut model, &mut transform_index);

  for node in collect_birdview_nodes(root) {
    let Some(tid) = transform_index.get(&node.fqn) else {
      continue;
    };
    for (idx, label) in birdview_state_labels(node).into_iter().enumerate() {
      let sid = format!("{tid}_s{idx}");
      model.states.push(BirdviewState { id: sid.clone(), label });
      model.edges.push(BirdviewEdge::HoldsState {
        transform: tid.clone(),
        state: sid,
      });
    }

    let mut seen_kinds: HashSet<String> = HashSet::new();
    for effect in &node.effects {
      if !seen_kinds.insert(effect.kind.clone()) {
        continue;
      }
      let eid = format!("{tid}_e_{}", mermaid_slug(&effect.kind));
      model.effects.push(BirdviewEffect {
        id: eid.clone(),
        kind: effect.kind.clone(),
      });
      model.edges.push(BirdviewEdge::Triggers {
        transform: tid.clone(),
        effect: eid,
      });
    }
  }

  model
}

fn collect_birdview_nodes(root: &EffectsGraphNode) -> Vec<&EffectsGraphNode> {
  let mut nodes = vec![root];
  let mut queue: Vec<&EffectsGraphNode> = root.children.iter().collect();
  while let Some(node) = queue.first().copied() {
    queue.remove(0);
    if node.seen {
      continue;
    }
    nodes.push(node);
    queue.extend(node.children.iter());
  }
  nodes
}

fn collect_birdview_transforms(node: &EffectsGraphNode, model: &mut BirdviewModel, index: &mut HashMap<String, String>) {
  if node.seen {
    return;
  }

  let collapsed = !is_analyzed_node(node);
  let tid = format!("t_{}", mermaid_slug(&node.fqn));
  let label = if collapsed {
    format!("{}<br/>…", mermaid_escape(&short_def_label(&node.fqn)))
  } else {
    mermaid_escape(&short_def_label(&node.fqn))
  };

  if !index.contains_key(&node.fqn) {
    index.insert(node.fqn.clone(), tid.clone());
    model.transforms.push(BirdviewTransform {
      id: tid.clone(),
      label,
      collapsed,
    });
  }

  let parent_tid = index.get(&node.fqn).cloned();
  for child in &node.children {
    if child.seen {
      continue;
    }
    collect_birdview_transforms(child, model, index);
    if let (Some(from), Some(to)) = (parent_tid.as_ref(), index.get(&child.fqn)) {
      let child_collapsed = !is_analyzed_node(child);
      model.edges.push(BirdviewEdge::Calls {
        from: from.clone(),
        to: to.clone(),
        collapsed: child_collapsed,
      });
    }
  }
}

fn birdview_state_labels(node: &EffectsGraphNode) -> Vec<String> {
  if !is_analyzed_node(node) {
    return vec![];
  }

  let mut labels = vec![];
  let mut has_atom_mut = false;

  for item in &node.state {
    match (item.kind.as_str(), item.name.as_str()) {
      ("atom" | "atom-write" | "atom-read", "defatom" | "reset!" | "swap!" | "deref" | "atom") => {
        has_atom_mut = true;
      }
      ("watch", _) => {}
      _ => labels.push(state_port_label(item)),
    }
  }

  if has_atom_mut {
    labels.push("atom<br/>mut".into());
  }

  labels.sort();
  labels.dedup();
  labels
}

fn state_port_label(item: &StateItem) -> String {
  let name = mermaid_escape(&item.name);
  let shape = item
    .type_hint
    .as_deref()
    .map(mermaid_escape)
    .unwrap_or_else(|| mermaid_escape(&item.kind));
  format!("{name}<br/>{shape}")
}

fn short_def_label(fqn: &str) -> String {
  fqn.rsplit('/').next().unwrap_or(fqn).to_string()
}

fn mermaid_slug(text: &str) -> String {
  text.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

fn mermaid_escape(text: &str) -> String {
  text.replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

fn render_mermaid_diagram(model: &BirdviewModel, entry: &str) -> String {
  let mut out = String::new();
  out.push_str("flowchart LR\n");
  out.push_str("  classDef stateNode fill:#dbeafe,stroke:#2563eb,color:#1e3a5f\n");
  out.push_str("  classDef transformNode fill:#fef9c3,stroke:#ca8a04,color:#451a03\n");
  out.push_str("  classDef effectNode fill:#fecaca,stroke:#dc2626,color:#450a0a\n\n");

  if model.transforms.is_empty() {
    out.push_str(&format!("  empty[[{entry}]]:::transformNode\n"));
    return out;
  }

  if !model.states.is_empty() {
    out.push_str("  subgraph stateLane[\"State\"]\n");
    out.push_str("    direction TB\n");
    for state in &model.states {
      out.push_str(&format!("    {}[\"{}\"]:::stateNode\n", state.id, state.label));
    }
    out.push_str("  end\n\n");
  }

  out.push_str("  subgraph transformLane[\"Transform\"]\n");
  out.push_str("    direction TB\n");
  for transform in &model.transforms {
    if transform.collapsed {
      out.push_str(&format!("    {}([\"{}\"]):::transformNode\n", transform.id, transform.label));
    } else {
      out.push_str(&format!("    {}[\"{}\"]:::transformNode\n", transform.id, transform.label));
    }
  }
  out.push_str("  end\n\n");

  if !model.effects.is_empty() {
    out.push_str("  subgraph effectLane[\"Effects\"]\n");
    out.push_str("    direction TB\n");
    for effect in &model.effects {
      out.push_str(&format!("    {}[[{}]]:::effectNode\n", effect.id, mermaid_escape(&effect.kind)));
    }
    out.push_str("  end\n\n");
  }

  for edge in &model.edges {
    match edge {
      BirdviewEdge::Calls { from, to, collapsed } => {
        let arrow = if *collapsed { "-.->|call|" } else { "-->|call|" };
        out.push_str(&format!("  {from} {arrow} {to}\n"));
      }
      BirdviewEdge::HoldsState { transform, state } => {
        out.push_str(&format!("  {transform} -.->|state| {state}\n"));
      }
      BirdviewEdge::Triggers { transform, effect } => {
        out.push_str(&format!("  {transform} ==>|effect| {effect}\n"));
      }
    }
  }

  out
}

fn format_birdview_legend(root: &EffectsGraphNode) -> String {
  let mut out = String::new();
  out.push_str("## Legend\n\n");
  out.push_str("- **State** (blue): data ports and structures\n");
  out.push_str("- **Transform** (yellow): key functions connecting nodes\n");
  out.push_str("- **Effect** (red): side-effect kinds triggered by transforms\n");
  out.push_str("- `-.->` state ownership · `-->` transform call · `==>` effect trigger\n\n");

  let child_targets = collect_child_targets(root);
  let collapsed: Vec<_> = child_targets.iter().filter(|t| !t.analyzed).collect();
  if !collapsed.is_empty() {
    out.push_str("### Expand\n\n");
    for target in collapsed {
      out.push_str(&format!("- `--root {fqn}`", fqn = target.fqn));
      if let Some(ref doc) = target.doc {
        out.push_str(&format!(" — {doc}"));
      }
      out.push('\n');
    }
  }
  out
}

// ═══════════════════════════════════════════════════════════════════════════════
// Program sketch (birdview text)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct SketchAtom {
  name: String,
  ns: String,
  type_hint: Option<String>,
}

#[derive(Debug, Clone)]
struct SketchHook {
  label: String,
  role: String,
}

#[derive(Debug, Default)]
struct ProgramSketch {
  entry: String,
  role: String,
  project_prefix: String,
  atoms: Vec<SketchAtom>,
  lifecycle: Vec<SketchHook>,
  channels: Vec<(String, Vec<String>)>,
  data_flow: Option<String>,
  expand: Vec<String>,
  namespaces: Vec<SketchNamespace>,
}

#[derive(Debug, Clone)]
struct SketchNamespace {
  ns: String,
  highlights: Vec<String>,
}

/// Aggregated birdview text — entry, project state, lifecycle, effect channels.
pub fn format_as_sketch(result: &EffectsGraphResult) -> String {
  let sketch = build_program_sketch(result);
  render_program_sketch(&sketch, result)
}

fn build_program_sketch(result: &EffectsGraphResult) -> ProgramSketch {
  let project_prefix = result
    .display
    .ns_prefix
    .clone()
    .unwrap_or_else(|| infer_project_prefix(&result.entry));

  let atoms = collect_project_atoms(&project_prefix);
  let mut lifecycle = collect_lifecycle_hooks(&result.tree, &project_prefix);
  supplement_entry_wiring(&result.entry, &mut lifecycle);
  let channels = collect_effect_channels(&result.tree, &project_prefix, 2);
  let data_flow = infer_data_flow(&lifecycle, &atoms, &channels);
  let role = infer_program_role(&result.tree, &result.entry);
  let expand = collect_expand_targets(&result.tree, &project_prefix);
  let namespaces = collect_project_map(&project_prefix);

  ProgramSketch {
    entry: result.entry.clone(),
    role,
    project_prefix,
    atoms,
    lifecycle,
    channels,
    data_flow,
    expand,
    namespaces,
  }
}

fn infer_project_prefix(entry: &str) -> String {
  let ns = entry.split('/').next().unwrap_or(entry);
  let head = ns.split('.').next().unwrap_or(ns);
  format!("{head}.")
}

fn collect_project_atoms(prefix: &str) -> Vec<SketchAtom> {
  let program_code = PROGRAM_CODE_DATA.read().ok();
  let Some(program_code) = program_code else {
    return vec![];
  };

  let mut atoms = vec![];
  for (ns, file) in program_code.iter() {
    if !ns.starts_with(prefix) {
      continue;
    }
    for (def, entry) in &file.defs {
      if !def.starts_with('*') {
        continue;
      }
      let type_hint = format_type_hint(&entry.schema).filter(|hint| hint != "dynamic");
      atoms.push(SketchAtom {
        name: def.to_string(),
        ns: ns.to_string(),
        type_hint,
      });
    }
  }

  atoms.sort_by(|a, b| a.name.cmp(&b.name));
  atoms
}

fn collect_lifecycle_hooks(root: &EffectsGraphNode, prefix: &str) -> Vec<SketchHook> {
  let mut hooks = vec![];
  for child in &root.children {
    if child.seen {
      continue;
    }
    if child.def.starts_with('*') {
      hooks.push(SketchHook {
        label: child.def.clone(),
        role: "state atom (referenced)".into(),
      });
      continue;
    }
    if !child.ns.starts_with(prefix) && !is_project_adjacent_ns(&child.ns, prefix) {
      continue;
    }
    hooks.push(SketchHook {
      label: short_def_label(&child.fqn),
      role: infer_hook_role(child, prefix),
    });
  }
  hooks
}

fn supplement_entry_wiring(entry: &str, hooks: &mut Vec<SketchHook>) {
  let Some((ns, def)) = entry.split_once('/') else {
    return;
  };
  let program_code = PROGRAM_CODE_DATA.read().ok();
  let Some(program_code) = program_code else {
    return;
  };
  let Some(file) = program_code.get(ns) else {
    return;
  };
  let Some(entry_def) = file.defs.get(def) else {
    return;
  };

  let mut existing: HashSet<String> = hooks.iter().map(|h| h.label.clone()).collect();
  scan_entry_wiring(&entry_def.code, hooks, &mut existing);
}

fn scan_entry_wiring(code: &Calcit, hooks: &mut Vec<SketchHook>, existing: &mut HashSet<String>) {
  match code {
    Calcit::List(list) => {
      if is_add_watch_head(list.first()) {
        if let Some(atom) = list.get(1) {
          push_atom_hook(atom, "watched state", hooks, existing);
        }
      }
      let _ = list.traverse_result::<String>(&mut |item| {
        scan_entry_wiring(item, hooks, existing);
        Ok(())
      });
    }
    Calcit::Fn { info, .. } => {
      for expr in info.body.iter() {
        scan_entry_wiring(expr, hooks, existing);
      }
    }
    Calcit::Symbol { sym, .. } => {
      let name = sym.as_ref();
      if name.starts_with('*') {
        if existing.insert(name.to_string()) {
          hooks.push(SketchHook {
            label: name.to_string(),
            role: "state reference".into(),
          });
        }
      } else if name.ends_with('!') && !matches!(name, "main!" | "reload!") && existing.insert(name.to_string()) {
        hooks.push(SketchHook {
          label: name.to_string(),
          role: "handler reference".into(),
        });
      }
    }
    Calcit::Import(import) if import.def.ends_with('!') || import.def.starts_with('*') => {
      let label = import.def.to_string();
      if existing.insert(label.clone()) {
        hooks.push(SketchHook {
          label,
          role: "imported handler".into(),
        });
      }
    }
    Calcit::Macro { info, .. } => {
      for expr in info.body.iter() {
        scan_entry_wiring(expr, hooks, existing);
      }
    }
    Calcit::Thunk(crate::calcit::CalcitThunk::Code { code, .. }) => {
      scan_entry_wiring(code, hooks, existing);
    }
    _ => {}
  }
}

fn is_add_watch_head(head: Option<&Calcit>) -> bool {
  match head {
    Some(Calcit::Proc(CalcitProc::AddWatch)) => true,
    Some(Calcit::Symbol { sym, .. }) => sym.as_ref() == "add-watch",
    _ => false,
  }
}

fn push_atom_hook(atom: &Calcit, role: &str, hooks: &mut Vec<SketchHook>, existing: &mut HashSet<String>) {
  if let Calcit::Symbol { sym, .. } = atom {
    if sym.starts_with('*') && existing.insert(sym.to_string()) {
      hooks.push(SketchHook {
        label: sym.to_string(),
        role: role.into(),
      });
    }
  }
}

fn is_project_adjacent_ns(ns: &str, prefix: &str) -> bool {
  ns.starts_with("reel.") || ns.starts_with(prefix)
}

fn infer_hook_role(node: &EffectsGraphNode, prefix: &str) -> String {
  if let Some(ref doc) = node.doc {
    let line = doc.lines().next().unwrap_or(doc).trim();
    if !line.is_empty() {
      return line.to_string();
    }
  }

  let def = node.def.as_str();
  let child_hints: Vec<String> = node
    .children
    .iter()
    .filter(|c| !c.seen && (c.ns.starts_with(prefix) || is_effect_named(&c.def)))
    .take(3)
    .map(|c| short_def_label(&c.fqn))
    .collect();

  let base = match def {
    n if n.contains("render") => "UI mount",
    n if n.contains("dispatch") => "state update handler",
    n if n.contains("persist") || n.contains("storage") => "persist to storage",
    n if n.contains("hydrate") => "hydrate from storage",
    n if n.contains("listen") => "event listener setup",
    n if n.contains("connect") => "external connection",
    n if n.starts_with("comp-") || n.contains("container") => "UI component",
    _ => "lifecycle",
  };

  if child_hints.is_empty() {
    base.into()
  } else {
    format!("{base} (→ {})", child_hints.join(", "))
  }
}

fn is_effect_named(def: &str) -> bool {
  def.ends_with('!') || def == "render!" || def.contains("send-to")
}

fn collect_effect_channels(root: &EffectsGraphNode, prefix: &str, max_depth: usize) -> Vec<(String, Vec<String>)> {
  let mut buckets: HashMap<String, HashSet<String>> = HashMap::new();
  collect_effect_channels_recursive(root, prefix, 0, max_depth, &mut buckets);

  let mut channels: Vec<(String, Vec<String>)> = buckets
    .into_iter()
    .map(|(channel, items)| {
      let mut list: Vec<String> = items.into_iter().collect();
      list.sort();
      (channel, list)
    })
    .collect();
  channels.sort_by(|a, b| a.0.cmp(&b.0));
  channels
}

fn collect_effect_channels_recursive(
  node: &EffectsGraphNode,
  prefix: &str,
  depth: usize,
  max_depth: usize,
  buckets: &mut HashMap<String, HashSet<String>>,
) {
  if depth > max_depth {
    return;
  }

  let include_effects = depth == 0 || depth == 1 || node.ns.starts_with(prefix);
  if include_effects {
    for effect in &node.effects {
      if effect.kind == "unknown/effect!" && !is_ui_proc(&effect.target) {
        continue;
      }
      let channel = effect_channel_name(&effect.kind, &effect.target);
      let label = effect_channel_label(&effect.kind, &effect.target);
      buckets.entry(channel).or_default().insert(label);
    }
  }

  if node.seen {
    return;
  }
  for child in &node.children {
    collect_effect_channels_recursive(child, prefix, depth + 1, max_depth, buckets);
  }
}

fn effect_channel_name(kind: &str, target: &str) -> String {
  if target.contains("localStorage") || matches!(kind, "io/write" | "io/read") && target.contains("storage") {
    return "Storage".into();
  }
  if target.contains("chrome") || target.contains("worker") || target.contains("extension") {
    return "Extension".into();
  }
  // UI channel: only show framework/UI procs, hide project lifecycle fns mis-tagged as unknown/effect!
  match kind {
    "console" => "Console".into(),
    "render" => "UI".into(),
    "unknown/effect!" if is_ui_proc(target) => "UI".into(),
    "io/read" | "io/write" | "io/file" | "io" => "Storage/IO".into(),
    "interop/js" => "DOM/JS".into(),
    "interop/host" | "interop/eval" => "Host".into(),
    "state/watch" => "Reactivity".into(),
    "env" => "Environment".into(),
    "async" => "Async".into(),
    _ if target.starts_with("js/") => "DOM/JS".into(),
    _ => "Other".into(),
  }
}

fn is_ui_proc(target: &str) -> bool {
  matches!(
    target,
    "render!" | "render-app!" | "send-to-component!" | "clear-cache!" | "mount-app!" | "rerender-app!"
  )
}

fn effect_channel_label(kind: &str, target: &str) -> String {
  if target == "println" || target == "eprintln" || target == "echo" {
    return target.to_string();
  }
  if kind == "unknown/effect!" || kind == "render" {
    return target.to_string();
  }
  if kind == "interop/js" && target.starts_with("js/") {
    return simplify_js_target(target);
  }
  if target.len() > 2 {
    return target.to_string();
  }
  kind.to_string()
}

fn simplify_js_target(target: &str) -> String {
  let rest = target.strip_prefix("js/").unwrap_or(target);
  if rest.len() > 40 {
    format!("js/{}…", &rest[..40])
  } else {
    target.to_string()
  }
}

fn infer_program_role(root: &EffectsGraphNode, entry: &str) -> String {
  if let Some(ref doc) = root.doc {
    let line = doc.lines().next().unwrap_or(doc).trim();
    if !line.is_empty() {
      return line.to_string();
    }
  }
  if entry.contains("comp.") || lifecycle_has(root, "render") {
    return "Respo UI application".into();
  }
  "Calcit application".into()
}

fn lifecycle_has(root: &EffectsGraphNode, needle: &str) -> bool {
  root.children.iter().any(|c| c.def.contains(needle))
}

fn infer_data_flow(lifecycle: &[SketchHook], atoms: &[SketchAtom], channels: &[(String, Vec<String>)]) -> Option<String> {
  let labels: HashSet<String> = lifecycle.iter().map(|h| h.label.clone()).collect();
  let has_reel = atoms.iter().any(|a| a.name.contains("reel") || a.name == "*reel");
  let has_dispatch = labels.iter().any(|l| l.contains("dispatch"));
  let has_render = labels.iter().any(|l| l.contains("render"));
  let has_watch = channels.iter().any(|(name, _)| name == "Reactivity");

  if has_dispatch && has_render && has_reel {
    return Some("event → dispatch! → *reel → [watch] → render-app! → render! → DOM".into());
  }
  if has_dispatch && has_render {
    return Some("event → dispatch! → state → [watch] → render-app! → render!".into());
  }
  if has_render && has_watch && has_reel {
    return Some("*reel change → [watch] → render-app! → render! → DOM".into());
  }
  if has_render && has_watch {
    return Some("state change → [watch] → render-app! → render!".into());
  }
  if lifecycle.len() >= 2 {
    let chain: Vec<String> = lifecycle
      .iter()
      .filter(|h| !h.label.starts_with('*'))
      .take(5)
      .map(|h| h.label.clone())
      .collect();
    if chain.len() >= 2 {
      return Some(chain.join(" → "));
    }
  }
  None
}

fn collect_expand_targets(root: &EffectsGraphNode, prefix: &str) -> Vec<String> {
  let mut targets = vec![];
  for child in &root.children {
    if child.seen {
      continue;
    }
    if child.def.starts_with('*') {
      continue;
    }
    if child.ns.starts_with(prefix) && (child.def.contains("comp-") || child.def.contains("container")) {
      targets.push(child.fqn.clone());
    }
    for grand in &child.children {
      if grand.seen {
        continue;
      }
      if grand.ns.starts_with(prefix) && (grand.def.contains("comp-") || grand.def.contains("container")) {
        targets.push(grand.fqn.clone());
      }
    }
  }
  targets.sort();
  targets.dedup();
  targets
}

fn collect_project_map(prefix: &str) -> Vec<SketchNamespace> {
  let program_code = PROGRAM_CODE_DATA.read().ok();
  let Some(program_code) = program_code else {
    return vec![];
  };

  let mut items = vec![];
  for (ns, file) in program_code.iter() {
    if !ns.starts_with(prefix) {
      continue;
    }
    let mut highlights: Vec<String> = file
      .defs
      .keys()
      .filter(|def| {
        def.starts_with('*')
          || def.ends_with('!')
          || def.contains("comp-")
          || def.contains("main")
          || def.contains("dispatch")
          || def.contains("updater")
      })
      .map(|def| def.to_string())
      .collect();
    highlights.sort();
    if highlights.len() > 8 {
      highlights.truncate(7);
      highlights.push("…".into());
    }
    items.push(SketchNamespace {
      ns: ns.to_string(),
      highlights,
    });
  }
  items.sort_by(|a, b| a.ns.cmp(&b.ns));
  items
}

fn render_program_sketch(sketch: &ProgramSketch, result: &EffectsGraphResult) -> String {
  let mut out = String::new();
  out.push_str(&format!("# Program Sketch: `{}`\n\n", sketch.entry));
  out.push_str(&format!("**Role:** {}\n", sketch.role));
  out.push_str(&format!("**Scope:** `{}*` (project definitions)\n\n", sketch.project_prefix));

  render_structure_section(result, &mut out);

  if !sketch.namespaces.is_empty() {
    out.push_str("## Project map\n\n");
    for item in &sketch.namespaces {
      if item.highlights.is_empty() {
        out.push_str(&format!("- `{}`\n", item.ns));
      } else {
        out.push_str(&format!("- `{}` — {}\n", item.ns, item.highlights.join(", ")));
      }
    }
    out.push('\n');
  }

  out.push_str("## State (persist)\n\n");
  if sketch.atoms.is_empty() {
    out.push_str("_No project-level atoms (`*name`) found._\n\n");
  } else {
    for atom in &sketch.atoms {
      let type_suffix = atom.type_hint.as_ref().map(|hint| format!(" — {hint}")).unwrap_or_default();
      out.push_str(&format!("- `{}` ({}){}\n", atom.name, atom.ns, type_suffix));
    }
    out.push('\n');
  }

  out.push_str("## Lifecycle (entry wiring)\n\n");
  if sketch.lifecycle.is_empty() {
    out.push_str("_No direct lifecycle hooks detected. Try `--max-depth 2`._\n\n");
  } else {
    for hook in &sketch.lifecycle {
      out.push_str(&format!("- `{}` — {}\n", hook.label, hook.role));
    }
    out.push('\n');
  }

  out.push_str("## Effects (channels)\n\n");
  if sketch.channels.is_empty() {
    out.push_str("_No effect channels detected at this depth._\n\n");
  } else {
    for (channel, items) in &sketch.channels {
      out.push_str(&format!("- **{channel}:** {}\n", items.join(", ")));
    }
    out.push('\n');
  }

  if let Some(ref flow) = sketch.data_flow {
    out.push_str("## Data flow (inferred)\n\n");
    out.push_str(&format!("{flow}\n\n"));
  }

  if !sketch.expand.is_empty() {
    out.push_str("## Expand\n\n");
    for fqn in &sketch.expand {
      out.push_str(&format!("- `cr ... analyze effects-graph --root {fqn} --max-depth 2`\n"));
    }
    out.push('\n');
  }

  out
}

fn render_structure_section(result: &EffectsGraphResult, out: &mut String) {
  out.push_str("## Structure\n\n");
  out.push_str("_Call tree from entry (same reachability as `cr analyze call-graph`)._\n\n");
  out.push_str("```\n");

  let hints = collect_effect_hints(&result.tree);
  if let Ok(call) = build_call_tree(result) {
    render_call_structure_node(&call.tree, out, "", true, &hints);
  } else {
    render_effects_structure_node(&result.tree, out, "", true);
  }

  out.push_str("```\n\n");
}

fn build_call_tree(result: &EffectsGraphResult) -> Result<crate::call_tree::CallTreeResult, String> {
  let parts: Vec<&str> = result.entry.split('/').collect();
  if parts.len() != 2 {
    return Err(format!("invalid entry: {}", result.entry));
  }
  analyze_call_graph(
    parts[0],
    parts[1],
    result.display.include_core,
    result.display.max_depth_limit,
    false,
    None,
    result.display.ns_prefix.clone(),
  )
}

fn collect_effect_hints(tree: &EffectsGraphNode) -> HashMap<String, String> {
  let mut hints = HashMap::new();
  collect_effect_hints_walk(tree, &mut hints);
  hints
}

fn collect_effect_hints_walk(node: &EffectsGraphNode, hints: &mut HashMap<String, String>) {
  let hint = structure_effect_hint(node);
  if !hint.is_empty() {
    hints.insert(node.fqn.clone(), hint);
  }
  for child in &node.children {
    collect_effect_hints_walk(child, hints);
  }
}

fn render_call_structure_node(
  node: &CallTreeNode,
  out: &mut String,
  prefix: &str,
  is_last: bool,
  hints: &HashMap<String, String>,
) {
  let connector = if is_last { "└── " } else { "├── " };
  let marker = if node.circular {
    " [circular]"
  } else if node.seen {
    " [seen]"
  } else if node.def.starts_with('*') {
    " [state]"
  } else if node.source == "core" {
    " [core]"
  } else if node.source == "external" {
    " [dep]"
  } else {
    ""
  };
  let effect = hints.get(&node.fqn).map(|h| format!("  · {h}")).unwrap_or_default();
  out.push_str(&format!("{prefix}{connector}{}{marker}{effect}\n", node.fqn));

  let child_prefix = format!("{prefix}{}   ", if is_last { " " } else { "│" });
  let child_count = node.calls.len();
  for (idx, child) in node.calls.iter().enumerate() {
    let is_last_child = idx + 1 == child_count;
    render_call_structure_node(child, out, &child_prefix, is_last_child, hints);
  }
}

fn render_effects_structure_node(node: &EffectsGraphNode, out: &mut String, prefix: &str, is_last: bool) {
  if node.seen {
    let connector = if is_last { "└── " } else { "├── " };
    out.push_str(&format!("{prefix}{connector}{} [seen]\n", node.fqn));
    return;
  }

  let connector = if is_last { "└── " } else { "├── " };
  let effects = structure_effect_hint(node);
  let effect = if effects.is_empty() {
    String::new()
  } else {
    format!("  · {effects}")
  };
  out.push_str(&format!("{prefix}{connector}{}{effect}\n", node.fqn));

  let child_prefix = format!("{prefix}{}   ", if is_last { " " } else { "│" });
  if node.circular {
    out.push_str(&format!("{child_prefix}└── [circular]\n"));
    return;
  }

  let child_count = node.children.len();
  for (idx, child) in node.children.iter().enumerate() {
    let is_last_child = idx + 1 == child_count;
    render_effects_structure_node(child, out, &child_prefix, is_last_child);
  }
}

fn structure_effect_hint(node: &EffectsGraphNode) -> String {
  let mut kinds: Vec<String> = node.effects.iter().map(|e| e.kind.clone()).collect();
  kinds.sort();
  kinds.dedup();
  if kinds.is_empty() {
    return String::new();
  }
  if kinds.len() > 3 {
    format!("{}, +{}", kinds[..3].join(", "), kinds.len() - 3)
  } else {
    kinds.join(", ")
  }
}

#[derive(Debug, Clone)]
struct ChildTarget {
  fqn: String,
  analyzed: bool,
  doc: Option<String>,
}

fn is_meaningful_call_target(ns: &str, def: &str, include_core: bool) -> bool {
  if matches!(
    def,
    "defn" | "defmacro" | "def" | "deftrait" | "defenum" | "defatom" | "reset!" | "swap!" | "deref" | "atom"
  ) {
    return false;
  }
  if !include_core && (ns == "calcit.core" || ns.starts_with("calcit.")) {
    return false;
  }
  true
}

fn is_analyzed_node(node: &EffectsGraphNode) -> bool {
  !node.state.is_empty()
    || !node.effects.is_empty()
    || !node.transform.summary.is_empty()
    || !node.transform.calls.is_empty()
    || !node.transform.control.is_empty()
    || !node.children.is_empty()
    || node.circular
}

fn count_subgraph_nodes(tree: &EffectsGraphNode) -> usize {
  collect_analyzed_subgraphs(tree).len()
}

fn collect_child_targets(tree: &EffectsGraphNode) -> Vec<ChildTarget> {
  tree
    .children
    .iter()
    .map(|child| ChildTarget {
      fqn: child.fqn.clone(),
      analyzed: is_analyzed_node(child) && !child.seen,
      doc: child.doc.clone(),
    })
    .collect()
}

fn collect_analyzed_subgraphs(tree: &EffectsGraphNode) -> Vec<&EffectsGraphNode> {
  let mut nodes = vec![];
  let mut queue: Vec<&EffectsGraphNode> = tree.children.iter().collect();
  while let Some(node) = queue.first().copied() {
    queue.remove(0);
    if is_analyzed_node(node) && !node.seen {
      nodes.push(node);
    }
    queue.extend(node.children.iter());
  }
  nodes
}

pub fn format_as_json(result: &EffectsGraphResult) -> Result<String, String> {
  serde_json::to_string_pretty(result).map_err(|e| format!("Failed to serialize to JSON: {e}"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::builtins::proc_tags;

  #[test]
  fn classify_read_file_by_name() {
    let kinds = classify_call("read-file", None);
    assert_eq!(kinds, vec!["io/read".to_string()]);
  }

  #[test]
  fn classify_console_from_log_tags() {
    let tags = proc_tags(["log", "io"]);
    let kinds = tags_to_effect_kinds(&tags);
    assert!(kinds.contains(&"console".to_string()));
  }

  #[test]
  fn sketch_format_contains_core_sections() {
    let result = EffectsGraphResult {
      entry: "app.main/main!".into(),
      tree: EffectsGraphNode {
        ns: "app.main".into(),
        def: "main!".into(),
        fqn: "app.main/main!".into(),
        doc: None,
        source: "project".into(),
        state: vec![],
        transform: TransformInfo::default(),
        effects: vec![EffectItem {
          kind: "console".into(),
          target: "println".into(),
          count: 1,
        }],
        children: vec![EffectsGraphNode {
          ns: "app.main".into(),
          def: "dispatch!".into(),
          fqn: "app.main/dispatch!".into(),
          doc: Some("state update handler".into()),
          source: "project".into(),
          state: vec![],
          transform: TransformInfo::default(),
          effects: vec![],
          children: vec![],
          circular: false,
          seen: false,
        }],
        circular: false,
        seen: false,
      },
      stats: EffectsGraphStats {
        reachable_count: 2,
        effect_sites: 1,
        state_items: 0,
        max_depth: 1,
        subgraph_count: 1,
      },
      display: EffectsGraphDisplayMeta {
        max_depth_limit: 2,
        detail: "summary".into(),
        include_core: false,
        ns_prefix: None,
      },
    };

    let text = format_as_sketch(&result);
    assert!(text.contains("# Program Sketch"));
    assert!(text.contains("## Structure"));
    assert!(text.contains("app.main/main!"));
    assert!(text.contains("## State (persist)"));
    assert!(text.contains("## Lifecycle (entry wiring)"));
    assert!(text.contains("## Effects (channels)"));
    assert!(text.contains("dispatch!"));
    assert!(text.contains("Console"));
  }

  #[test]
  fn mermaid_birdview_shows_state_transform_effect_links() {
    let result = EffectsGraphResult {
      entry: "app.main/main!".into(),
      tree: EffectsGraphNode {
        ns: "app.main".into(),
        def: "main!".into(),
        fqn: "app.main/main!".into(),
        doc: None,
        source: "project".into(),
        state: vec![],
        transform: TransformInfo::default(),
        effects: vec![EffectItem {
          kind: "console".into(),
          target: "println".into(),
          count: 1,
        }],
        children: vec![EffectsGraphNode {
          ns: "app.main".into(),
          def: "helper".into(),
          fqn: "app.main/helper".into(),
          doc: None,
          source: "project".into(),
          state: vec![StateItem {
            kind: "param".into(),
            name: "data".into(),
            type_hint: Some(":map".into()),
          }],
          transform: TransformInfo::default(),
          effects: vec![EffectItem {
            kind: "io/read".into(),
            target: "read-file".into(),
            count: 1,
          }],
          children: vec![],
          circular: false,
          seen: false,
        }],
        circular: false,
        seen: false,
      },
      stats: EffectsGraphStats {
        reachable_count: 2,
        effect_sites: 2,
        state_items: 1,
        max_depth: 1,
        subgraph_count: 1,
      },
      display: EffectsGraphDisplayMeta {
        max_depth_limit: 0,
        detail: "summary".into(),
        include_core: false,
        ns_prefix: None,
      },
    };

    let mermaid = format_as_mermaid(&result);
    assert!(mermaid.contains("```mermaid"));
    assert!(mermaid.contains("stateNode"));
    assert!(mermaid.contains("transformNode"));
    assert!(mermaid.contains("effectNode"));
    assert!(mermaid.contains("data&lt;br/&gt;:map") || mermaid.contains("data<br/>:map"));
    assert!(mermaid.contains("-->|call|"));
    assert!(mermaid.contains("==>|effect|"));
    assert!(mermaid.contains("console"));
    assert!(mermaid.contains("io/read"));
  }

  #[test]
  fn heuristic_detects_js_prefix() {
    let kinds = heuristic_effect_kinds("js/console.log");
    assert_eq!(kinds, vec!["interop/js".to_string()]);
  }
}
