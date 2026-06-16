//! Effects graph analysis: State / Transform / Effect decomposition per definition.

use crate::builtins;
use crate::calcit::{Calcit, CalcitFnArgs, CalcitLocal, CalcitProc, CalcitSyntax};
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
  /// True when max_depth limit prevented expanding children (analysis still present).
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  pub depth_exceeded: bool,
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
        depth_exceeded: false,
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
        depth_exceeded: false,
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

    let depth_exceeded = self.config.max_depth > 0 && depth >= self.config.max_depth;

    if let Some(code) = code {
      // Always analyze the code for state/effects/transform, even at max depth
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

      // Only build children if not at max depth
      if !depth_exceeded {
        for (call_ns, call_def) in call_refs {
          if !self.config.include_core && self.is_core_ns(&call_ns) {
            continue;
          }
          if call_ns == ns && call_def == def {
            continue;
          }
          children.push(self.build_node(&call_ns, &call_def, depth + 1)?);
        }
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
      depth_exceeded,
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
        self.inspect_call_head(head, Some(list), current_ns, out);
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

  fn inspect_call_head(&self, head: &Calcit, list: Option<&crate::calcit::CalcitList>, current_ns: &str, out: &mut DefAnalysis) {
    let Some((name, ns_hint)) = call_operator(head, current_ns) else {
      return;
    };

    if is_state_operator(&name) {
      let target = extract_state_target(list, &name);
      record_state_operator(&name, &target, &mut out.state);
      if matches!(name.as_str(), "defatom" | "atom" | "reset!" | "swap!" | "deref" | "set!") {
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
    "defatom" | "reset!" | "swap!" | "atom" | "deref" | "add-watch" | "remove-watch" | "set!"
  )
}

fn record_state_operator(op_name: &str, target: &str, state: &mut Vec<StateItem>) {
  let kind = match op_name {
    "defatom" | "atom" => "atom",
    "reset!" | "swap!" => "atom-write",
    "add-watch" | "remove-watch" => "watch",
    "deref" => "atom-read",
    "set!" => "local-write",
    _ => "state",
  };
  state.push(StateItem {
    kind: kind.into(),
    name: target.into(),
    type_hint: None,
  });
}

/// Extract the atom name from a state-operator call like `(swap! *store ...)`.
fn extract_state_target(list: Option<&crate::calcit::CalcitList>, op_name: &str) -> String {
  let Some(list) = list else {
    return op_name.to_string();
  };
  match op_name {
    "swap!" | "reset!" | "deref" | "add-watch" | "remove-watch" | "set!" => {
      list.get(1).and_then(extract_symbol_name).unwrap_or_else(|| op_name.to_string())
    }
    "defatom" | "atom" => list.get(1).and_then(extract_symbol_name).unwrap_or_else(|| "?".to_string()),
    _ => op_name.to_string(),
  }
}

fn extract_symbol_name(calcit: &Calcit) -> Option<String> {
  match calcit {
    Calcit::Symbol { sym, .. } => Some(sym.to_string()),
    Calcit::Str(s) => Some(s.to_string()),
    _ => None,
  }
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
    // Respo convention: common project-level functions
    "render-app!" | "mount-app!" | "rerender-app!" | "clear-cache!" => vec!["render"],
    "send-to-component!" | "dispatch!" => vec!["lifecycle"],
    "save-store!" => vec!["storage"],
    "realize-ssr!" => vec!["render"],
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
  // Common project-level effect conventions
  if name.contains("load") || name.contains("init") || name.contains("setup") {
    return vec!["io".into()];
  }
  if name.ends_with('!') && !matches!(name, "main!" | "reload!" | "quit!" | "reset!" | "swap!") {
    return vec!["effect".into()];
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
    || node.depth_exceeded
}

fn count_subgraph_nodes(tree: &EffectsGraphNode) -> usize {
  collect_analyzed_subgraphs(tree).len()
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

// ═══════════════════════════════════════════════════════════════════════════════
// STE tree format — per-function State / Transform / Effect decomposition
// ═══════════════════════════════════════════════════════════════════════════════

/// Render per-function STE decomposition tree.
pub fn format_as_ste_tree(result: &EffectsGraphResult) -> String {
  let mut out = String::new();
  out.push_str(&format!("# Effects Graph: `{}`\n\n", result.entry));

  let depth_limited = count_depth_limited(&result.tree);
  if depth_limited > 0 {
    out.push_str(&format!(
      "Max depth: {}  ({} nodes truncated; rerun with larger --max-depth to expand)\n\n",
      result.display.max_depth_limit, depth_limited
    ));
  }

  render_ste_node(&result.tree, &mut out, "", true);
  out
}

fn count_depth_limited(node: &EffectsGraphNode) -> usize {
  let mut count = if node.depth_exceeded { 1 } else { 0 };
  for child in &node.children {
    count += count_depth_limited(child);
  }
  count
}

fn render_ste_node(node: &EffectsGraphNode, out: &mut String, prefix: &str, is_last: bool) {
  if node.seen {
    return;
  }

  let connector = if is_last { "└── " } else { "├── " };
  let (kind_label, is_collapsed) = ste_kind_label(node);
  let summary = node_transform_summary(node);

  out.push_str(&format!("{prefix}{connector}{}  {kind_label}\n", node.fqn));

  let child_prefix = format!("{prefix}{}   ", if is_last { " " } else { "│" });

  if is_collapsed {
    if !summary.is_empty() {
      out.push_str(&format!("{child_prefix}({summary})\n"));
    }
    return;
  }

  render_ste_state(node, out, &child_prefix);
  render_ste_transform(node, out, &child_prefix);
  render_ste_effects(node, out, &child_prefix);

  if !node.children.is_empty() {
    out.push_str(&format!("{child_prefix}│\n"));
  }
  for (idx, child) in node.children.iter().enumerate() {
    let is_last_child = idx + 1 == node.children.len();
    render_ste_node(child, out, &child_prefix, is_last_child);
  }
}

fn ste_kind_label(node: &EffectsGraphNode) -> (&'static str, bool) {
  if node.depth_exceeded {
    ("[depth limit ↑]", true)
  } else if node.circular {
    ("[circular]", true)
  } else if node.source == "core" {
    ("[core]", true)
  } else if !is_analyzed_node(node) {
    ("[no analysis]", true)
  } else if node.def.starts_with('*') {
    ("[state]", false)
  } else if node.effects.is_empty() {
    ("[transform]", false)
  } else {
    ("[program]", false)
  }
}

fn node_transform_summary(node: &EffectsGraphNode) -> String {
  let mut parts: Vec<String> = vec![];

  let doc_line = node.doc.as_ref().and_then(|doc| {
    let line = doc.lines().next().unwrap_or(doc).trim();
    if line.is_empty() { None } else { Some(line.to_string()) }
  });

  if let Some(ref line) = doc_line {
    parts.push(line.clone());
  }

  let summary = &node.transform.summary;
  if !summary.is_empty() && summary != "transform" && doc_line.as_ref() != Some(summary) {
    parts.push(summary.clone());
  }

  if !node.effects.is_empty() {
    let kinds: Vec<String> = node.effects.iter().map(|e| e.kind.clone()).collect();
    let mut unique: Vec<String> = kinds;
    unique.sort();
    unique.dedup();
    let effect_part = format!("effects: {}", unique.join(", "));
    if !parts.iter().any(|p| p.contains("effects:")) {
      parts.push(effect_part);
    }
  }

  let joined = parts.join("; ");
  if joined.len() > 120 {
    format!("{}…", &joined[..117])
  } else {
    joined
  }
}

fn render_ste_state(node: &EffectsGraphNode, out: &mut String, prefix: &str) {
  if node.state.is_empty() {
    return;
  }
  out.push_str(&format!("{prefix}├── State\n"));
  let item_prefix = format!("{prefix}│   ");

  let mut params: Vec<&StateItem> = vec![];
  let mut atoms: Vec<&StateItem> = vec![];
  let mut returns: Vec<&StateItem> = vec![];
  let mut watches: Vec<&StateItem> = vec![];
  let mut others: Vec<&StateItem> = vec![];

  for item in &node.state {
    match item.kind.as_str() {
      "param" => params.push(item),
      "atom" | "atom-read" | "atom-write" => atoms.push(item),
      "return" => returns.push(item),
      "watch" => watches.push(item),
      _ => others.push(item),
    }
  }

  let mut all_items: Vec<String> = vec![];
  for item in &params {
    all_items.push(format!("param    {}", ste_state_detail(item)));
  }
  for item in &atoms {
    all_items.push(format!("atom     {}", ste_state_detail(item)));
  }
  for item in &returns {
    all_items.push(format!("return   {}", ste_state_detail(item)));
  }
  for item in &watches {
    all_items.push(format!("watch    {}", ste_state_detail(item)));
  }
  for item in &others {
    all_items.push(format!("{:<8} {}", item.kind, ste_state_detail(item)));
  }
  if all_items.is_empty() {
    all_items.push("(none)".into());
  }

  for (idx, line) in all_items.iter().enumerate() {
    let conn = if idx + 1 == all_items.len() { "└── " } else { "├── " };
    out.push_str(&format!("{item_prefix}{conn}{line}\n"));
  }
}

fn ste_state_detail(item: &StateItem) -> String {
  if item.kind == "return" {
    return item.type_hint.as_deref().unwrap_or(&item.name).to_string();
  }
  let mut s = item.name.clone();
  if item.kind == "atom-write" {
    s.push_str(" (write)");
  } else if item.kind == "atom-read" {
    s.push_str(" (read)");
  } else if item.kind == "local-write" {
    s.push_str(" (set!)");
  }
  if let Some(ref hint) = item.type_hint {
    s.push_str(&format!("  :{hint}"));
  }
  s
}

fn render_ste_transform(node: &EffectsGraphNode, out: &mut String, prefix: &str) {
  let has_control = !node.transform.control.is_empty();
  let has_calls = !node.transform.calls.is_empty();
  let summary = if node.transform.summary.is_empty() || node.transform.summary == "transform" {
    String::new()
  } else {
    format!("  ({})", node.transform.summary)
  };

  if !has_control && !has_calls {
    out.push_str(&format!("{prefix}├── Transform{summary}\n"));
    out.push_str(&format!("{prefix}│   └── (no calls)\n"));
    return;
  }

  out.push_str(&format!("{prefix}├── Transform{summary}\n"));
  let item_prefix = format!("{prefix}│   ");
  let mut lines: Vec<String> = vec![];

  if has_control {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for item in &node.transform.control {
      *counts.entry(item.as_str()).or_default() += 1;
    }
    let control_summary: Vec<String> = counts
      .into_iter()
      .map(|(name, count)| if count > 1 { format!("{name}×{count}") } else { name.to_string() })
      .collect();
    lines.push(format!("control: {}", control_summary.join(", ")));
  }

  for call in &node.transform.calls {
    lines.push(format!("→ {call}"));
  }

  for (idx, line) in lines.iter().enumerate() {
    let conn = if idx + 1 == lines.len() { "└── " } else { "├── " };
    out.push_str(&format!("{item_prefix}{conn}{line}\n"));
  }
}

fn render_ste_effects(node: &EffectsGraphNode, out: &mut String, prefix: &str) {
  if node.effects.is_empty() {
    out.push_str(&format!("{prefix}└── Effects\n"));
    out.push_str(&format!("{prefix}    └── (none — pure transform)\n"));
    return;
  }

  out.push_str(&format!("{prefix}└── Effects\n"));
  let item_prefix = format!("{prefix}    ");

  let mut seen: HashSet<String> = HashSet::new();
  let mut unique_effects: Vec<&EffectItem> = vec![];
  for effect in &node.effects {
    if seen.insert(format!("{}::{}", effect.kind, effect.target)) {
      unique_effects.push(effect);
    }
  }
  unique_effects.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.target.cmp(&b.target)));

  for (idx, effect) in unique_effects.iter().enumerate() {
    let conn = if idx + 1 == unique_effects.len() {
      "└── "
    } else {
      "├── "
    };
    let count_suffix = if effect.count > 1 {
      format!(" (×{})", effect.count)
    } else {
      String::new()
    };
    out.push_str(&format!("{item_prefix}{conn}{:<14} {}{count_suffix}\n", effect.kind, effect.target));
  }
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
  fn heuristic_detects_js_prefix() {
    let kinds = heuristic_effect_kinds("js/console.log");
    assert_eq!(kinds, vec!["interop/js".to_string()]);
  }

  #[test]
  fn ste_tree_contains_core_sections() {
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
          depth_exceeded: false,
        }],
        circular: false,
        seen: false,
        depth_exceeded: false,
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

    let text = format_as_ste_tree(&result);
    assert!(text.contains("# Effects Graph"));
    assert!(text.contains("app.main/main!"));
    assert!(text.contains("console"));
    assert!(text.contains("println"));
    assert!(text.contains("[program]"));
  }
}
