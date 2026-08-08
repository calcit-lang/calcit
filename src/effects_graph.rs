//! Effects graph analysis: State / Transform / Effect decomposition per definition.

use crate::builtins;
use crate::calcit::CalcitTypeAnnotation;
use crate::calcit::{Calcit, CalcitFnArgs, CalcitLocal, CalcitProc, CalcitStructDef, CalcitSyntax};
use crate::program::{
  ImportRule, PROGRAM_CODE_DATA, lookup_codegen_type_hint, lookup_def_code, lookup_def_schema, lookup_ns_target_in_import,
};
use cirru_edn::{Edn, EdnListView, EdnMapView, EdnTag};
use colored::Colorize;
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
  /// package root inferred from entry or `--ns-prefix`, used to shorten display paths
  pub package_root: Option<String>,
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

    let package_root = infer_package_root(entry_ns, &self.config.ns_prefix);

    let display = EffectsGraphDisplayMeta {
      max_depth_limit: self.config.max_depth,
      detail: match self.config.detail {
        EffectsGraphDetail::Full => "full".into(),
        EffectsGraphDetail::Minimal => "minimal".into(),
        EffectsGraphDetail::Summary => "summary".into(),
      },
      include_core: self.config.include_core,
      ns_prefix: self.config.ns_prefix.clone(),
      package_root,
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
      if def.starts_with('*') {
        enrich_atom_def_state(ns, def, &mut analysis.state);
      }
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
      Calcit::Enum(enum_value) => {
        for item in &enum_value.extra {
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
      record_state_operator(&name, &target, list, current_ns, &mut out.state);
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
      if current_ns != "calcit.core"
        && let Some(core) = program_code.get("calcit.core")
        && core.defs.contains_key(sym.as_ref())
      {
        return Some((sym.to_string(), Some("calcit.core".into())));
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
      if current_ns != "calcit.core"
        && let Some(core) = program_code.get("calcit.core")
        && core.defs.contains_key(sym.as_ref())
      {
        return Some(("calcit.core".into(), sym.to_string()));
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
    Calcit::Enum(enum_value) => {
      for item in &enum_value.extra {
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

fn record_state_operator(
  op_name: &str,
  target: &str,
  list: Option<&crate::calcit::CalcitList>,
  current_ns: &str,
  state: &mut Vec<StateItem>,
) {
  let kind = match op_name {
    "defatom" | "atom" => "atom-def",
    "reset!" | "swap!" => "atom-write",
    "add-watch" | "remove-watch" => "watch",
    "deref" => "atom-read",
    "set!" => "local-write",
    _ => "state",
  };
  let type_hint = if kind == "atom-def" {
    list
      .and_then(|items| items.get(2))
      .and_then(|init| summarize_init_expr(init, current_ns))
      .map(|raw| finalize_schema_hint(&raw, current_ns))
      .or_else(|| lookup_atom_schema(current_ns, target))
  } else if kind == "watch" && target.starts_with('*') {
    lookup_atom_schema(current_ns, target)
  } else {
    None
  };
  state.push(StateItem {
    kind: kind.into(),
    name: target.into(),
    type_hint,
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

fn format_type_hint(schema: &std::sync::Arc<CalcitTypeAnnotation>) -> Option<String> {
  format_schema_annotation(schema, "", MAX_SCHEMA_DEPTH)
}

const MAX_SCHEMA_DEPTH: usize = 4;

fn is_informative_schema(hint: &str) -> bool {
  !matches!(
    hint,
    "dynamic" | ":dynamic" | "ref" | ":ref" | "ref<:dynamic>" | "map<dynamic,dynamic>" | "map<:dynamic,:dynamic>"
  )
}

fn is_coarse_schema_hint(hint: &str) -> bool {
  matches!(
    hint,
    "dynamic" | ":dynamic" | "ref" | ":ref" | "ref<:dynamic>" | "map<dynamic,dynamic>" | "map<:dynamic,:dynamic>" | ":map"
  )
}

fn finalize_schema_hint(raw: &str, context_ns: &str) -> String {
  if raw.starts_with('{') || raw.contains(":states") || raw.contains(":records") {
    return raw.to_string();
  }
  expand_type_ref_path(raw, context_ns, MAX_SCHEMA_DEPTH).unwrap_or_else(|| raw.to_string())
}

fn resolve_type_ref_path(ref_path: &str, context_ns: &str) -> Option<(String, String)> {
  let (ns_part, def) = ref_path.split_once('/')?;
  if let Some(real_ns) = lookup_ns_target_in_import(context_ns, ns_part) {
    return Some((real_ns.to_string(), def.to_string()));
  }
  let program = PROGRAM_CODE_DATA.read().ok()?;
  if program.get(ns_part).is_some() {
    return Some((ns_part.to_string(), def.to_string()));
  }
  if ns_part.contains('.') {
    return Some((ns_part.to_string(), def.to_string()));
  }
  let dotted = format!("{context_ns}.{ns_part}");
  if program.get(dotted.as_str()).is_some() {
    return Some((dotted, def.to_string()));
  }
  None
}

fn expand_type_ref_path(ref_path: &str, context_ns: &str, depth: usize) -> Option<String> {
  if depth == 0 {
    return None;
  }
  let (ns, def) = resolve_type_ref_path(ref_path, context_ns)?;
  let template = expand_from_def_template(&ns, &def, context_ns, depth);
  let annotated = expand_from_type_annotation(&ns, &def, context_ns, depth);
  match (template, annotated) {
    (Some(t), Some(a)) if is_coarse_schema_hint(&a) => Some(t),
    (Some(t), Some(a)) => Some(pick_richer_schema(Some(&a), &t)),
    (Some(t), None) => Some(t),
    (None, Some(a)) => Some(a),
    (None, None) => None,
  }
}

fn expand_from_type_annotation(ns: &str, def: &str, context_ns: &str, depth: usize) -> Option<String> {
  let schema = lookup_codegen_type_hint(ns, def).unwrap_or_else(|| lookup_def_schema(ns, def));
  format_schema_annotation(&schema, context_ns, depth)
}

fn format_schema_annotation(schema: &std::sync::Arc<CalcitTypeAnnotation>, context_ns: &str, depth: usize) -> Option<String> {
  if depth == 0 {
    return None;
  }
  match schema.as_ref() {
    CalcitTypeAnnotation::Dynamic => None,
    CalcitTypeAnnotation::Ref(inner) => format_schema_annotation(inner, context_ns, depth - 1),
    CalcitTypeAnnotation::TypeRef(name, _) => {
      let stripped = name.trim_start_matches('\'').trim_start_matches(':');
      expand_type_ref_path(stripped, context_ns, depth - 1)
    }
    CalcitTypeAnnotation::Fn(fn_type) => Some(fn_type.return_type.to_brief_string()),
    CalcitTypeAnnotation::Map(key, value)
      if matches!(key.as_ref(), CalcitTypeAnnotation::Dynamic) && matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic) =>
    {
      None
    }
    CalcitTypeAnnotation::Map(key, value) => {
      let k = format_schema_annotation(key, context_ns, depth - 1).unwrap_or_else(|| key.to_brief_string());
      let v = format_schema_annotation(value, context_ns, depth - 1).unwrap_or_else(|| value.to_brief_string());
      Some(format!("map<{k},{v}>"))
    }
    CalcitTypeAnnotation::List(inner) => format_schema_annotation(inner, context_ns, depth - 1).map(|t| format!("list<{t}>")),
    CalcitTypeAnnotation::Optional(inner) => format_schema_annotation(inner, context_ns, depth - 1).map(|t| format!("{t}?")),
    CalcitTypeAnnotation::JsNullish(inner) => {
      format_schema_annotation(inner, context_ns, depth - 1).map(|t| format!("js-nullish<{t}>"))
    }
    _ => {
      if let Some(st) = schema.resolve_to_struct() {
        return Some(format_struct_fields(&st, context_ns, depth));
      }
      let brief = schema.to_brief_string();
      if is_informative_schema(&brief) { Some(brief) } else { None }
    }
  }
}

fn format_struct_fields(st: &CalcitStructDef, context_ns: &str, depth: usize) -> String {
  let fields: Vec<String> = st
    .fields
    .iter()
    .zip(st.field_types.iter())
    .map(|(field, ty)| {
      let ty_str = format_schema_annotation(ty, context_ns, depth.saturating_sub(1)).unwrap_or_else(|| ty.to_brief_string());
      format!(":{field} {ty_str}")
    })
    .collect();
  if fields.is_empty() {
    format!("struct {}", st.name)
  } else {
    format!("{{ {} }}", fields.join(", "))
  }
}

fn expand_from_def_template(ns: &str, def: &str, context_ns: &str, depth: usize) -> Option<String> {
  if depth == 0 {
    return None;
  }
  let code = lookup_def_code(ns, def)?;
  let fields = find_best_struct_template(&code)?;
  Some(format_struct_template_fields(&fields, context_ns, depth - 1))
}

fn find_best_struct_template(code: &Calcit) -> Option<Vec<(String, String)>> {
  let mut best: Option<Vec<(String, String)>> = None;
  walk_for_struct_templates(code, &mut best);
  best
}

fn walk_for_struct_templates(code: &Calcit, best: &mut Option<Vec<(String, String)>>) {
  if let Some(fields) = parse_struct_field_pairs(code)
    && fields.len() >= 2
    && best.as_ref().is_none_or(|current| current.len() < fields.len())
  {
    *best = Some(fields);
  }
  match code {
    Calcit::List(list) => {
      for item in list.iter() {
        walk_for_struct_templates(item, best);
      }
    }
    Calcit::Fn { info, .. } => {
      for expr in info.body.iter() {
        walk_for_struct_templates(expr, best);
      }
    }
    Calcit::Macro { info, .. } => {
      for expr in info.body.iter() {
        walk_for_struct_templates(expr, best);
      }
    }
    Calcit::Thunk(crate::calcit::CalcitThunk::Code { code, .. }) => walk_for_struct_templates(code, best),
    Calcit::Struct(struct_value) => {
      if let Some(fields) = struct_fields_from_calcit(struct_value)
        && fields.len() >= 2
        && best.as_ref().is_none_or(|current| current.len() < fields.len())
      {
        *best = Some(fields);
      }
    }
    Calcit::Map(map) => {
      for (k, v) in map.iter() {
        walk_for_struct_templates(k, best);
        walk_for_struct_templates(v, best);
      }
    }
    _ => {}
  }
}

fn format_struct_template_fields(fields: &[(String, String)], context_ns: &str, depth: usize) -> String {
  let parts: Vec<String> = fields
    .iter()
    .map(|(key, hint)| {
      let expanded = if depth > 0 && hint.contains('/') {
        finalize_schema_hint(hint, context_ns)
      } else {
        hint.clone()
      };
      format!(":{key} {expanded}")
    })
    .collect();
  format!("{{ {} }}", parts.join(", "))
}

fn parse_struct_field_pairs(expr: &Calcit) -> Option<Vec<(String, String)>> {
  if let Calcit::Struct(struct_value) = expr {
    return struct_fields_from_calcit(struct_value);
  }
  let Calcit::List(list) = expr else {
    return None;
  };
  let start = if is_struct_literal_head(list.first()) { 1 } else { 0 };
  parse_flat_struct_pairs(list, start).or_else(|| parse_nested_struct_entries(list, start))
}

fn parse_flat_struct_pairs(list: &crate::calcit::CalcitList, start: usize) -> Option<Vec<(String, String)>> {
  if start >= list.len() {
    return None;
  }
  extract_field_key(list.get(start)?)?;
  let mut fields = vec![];
  let mut idx = start;
  while idx + 1 < list.len() {
    let key = extract_field_key(list.get(idx)?)?;
    let value_hint = value_shape_hint(list.get(idx + 1)?);
    fields.push((key, value_hint));
    idx += 2;
  }
  if fields.is_empty() { None } else { Some(fields) }
}

fn parse_nested_struct_entries(list: &crate::calcit::CalcitList, start: usize) -> Option<Vec<(String, String)>> {
  let mut fields = vec![];
  for idx in start..list.len() {
    let Some(entry) = list.get(idx) else { continue };
    let Calcit::List(entry_list) = entry else { continue };
    if entry_list.len() < 2 {
      continue;
    }
    let key = extract_field_key(entry_list.get(0)?)?;
    let value_hint = value_shape_hint(entry_list.get(1)?);
    fields.push((key, value_hint));
  }
  if fields.is_empty() { None } else { Some(fields) }
}

fn struct_fields_from_calcit(struct_value: &crate::calcit::CalcitStructValue) -> Option<Vec<(String, String)>> {
  let fields = struct_value.fields();
  let values = struct_value.values.as_ref();
  if fields.is_empty() || fields.len() != values.len() {
    return None;
  }
  Some(
    fields
      .iter()
      .zip(values.iter())
      .map(|(field, value)| (field.to_string(), value_shape_hint(value)))
      .collect(),
  )
}

fn is_struct_literal_head(head: Option<&Calcit>) -> bool {
  match head {
    Some(Calcit::Proc(CalcitProc::NativeLooseStruct)) => true,
    Some(Calcit::Proc(CalcitProc::NativeEnum)) => true,
    Some(Calcit::Symbol { sym, .. }) => matches!(sym.as_ref(), "{}" | "?{}" | "::"),
    _ => false,
  }
}

fn extract_field_key(calcit: &Calcit) -> Option<String> {
  match calcit {
    Calcit::Tag(tag) => Some(tag.to_string()),
    Calcit::Symbol { sym, .. } if sym.starts_with(':') => Some(sym[1..].to_string()),
    Calcit::Symbol { sym, .. } => Some(sym.to_string()),
    Calcit::Str(s) => Some(s.to_string()),
    _ => None,
  }
}

fn value_shape_hint(value: &Calcit) -> String {
  match value {
    Calcit::Nil => ":nil".into(),
    Calcit::Bool(_) => ":bool".into(),
    Calcit::Number(_) => ":number".into(),
    Calcit::Str(_) => ":string".into(),
    Calcit::Tag(_) => ":tag".into(),
    Calcit::Import(import) => format!("{}/{}", import.ns, import.def),
    Calcit::Symbol { sym, .. } if sym.contains('/') => sym.to_string(),
    Calcit::Map(map) if map.is_empty() => ":map {}".into(),
    Calcit::List(list) if list.is_empty() => ":list []".into(),
    Calcit::List(list) if list.len() == 1 && matches!(list.first(), Some(Calcit::Proc(CalcitProc::List))) => ":list []".into(),
    Calcit::List(_) => parse_struct_field_pairs(value)
      .map(|fields| format_struct_template_fields(&fields, "", 1))
      .unwrap_or_else(|| ":struct".into()),
    _ => "…".into(),
  }
}

fn lookup_atom_schema(context_ns: &str, atom_name: &str) -> Option<String> {
  if !atom_name.starts_with('*') {
    return None;
  }
  let program_code = PROGRAM_CODE_DATA.read().ok()?;
  let mut best: Option<String> = None;
  if program_code.get(context_ns).is_some_and(|file| file.defs.contains_key(atom_name))
    && let Some(hint) = schema_hint_for_atom_def(context_ns, atom_name, context_ns)
  {
    best = Some(pick_richer_schema(best.as_deref(), &hint));
  }
  for (ns, file) in program_code.iter() {
    if ns.as_ref() != context_ns
      && file.defs.contains_key(atom_name)
      && let Some(hint) = schema_hint_for_atom_def(ns, atom_name, context_ns)
    {
      best = Some(pick_richer_schema(best.as_deref(), &hint));
    }
  }
  best.filter(|hint| is_informative_schema(hint))
}

fn pick_richer_option(current: Option<&str>, next: Option<&str>) -> Option<String> {
  match (current, next) {
    (Some(cur), Some(nxt)) => Some(pick_richer_schema(Some(cur), nxt)),
    (Some(cur), None) => Some(cur.to_string()),
    (None, Some(nxt)) => Some(nxt.to_string()),
    (None, None) => None,
  }
}

struct MergedStateItem {
  key: String,
  kind_label: String,
  kind: String,
  name: String,
  type_hint: Option<String>,
  count: usize,
}

fn state_item_key(item: &StateItem) -> String {
  format!("{}::{}", item.kind, item.name)
}

fn state_kind_label(kind: &str) -> &'static str {
  match kind {
    "param" => "in",
    "return" => "out",
    "atom-def" => "define",
    "atom" | "atom-read" | "atom-write" => "atom",
    "watch" => "watch",
    _ => "state",
  }
}

fn merge_state_items(items: Vec<(&str, &StateItem)>) -> Vec<MergedStateItem> {
  let mut merged: Vec<MergedStateItem> = vec![];
  for (label, item) in items {
    let key = state_item_key(item);
    if let Some(entry) = merged.iter_mut().find(|entry| entry.key == key) {
      entry.count += 1;
      entry.type_hint = pick_richer_option(entry.type_hint.as_deref(), item.type_hint.as_deref());
      continue;
    }
    merged.push(MergedStateItem {
      key,
      kind_label: label.to_string(),
      kind: item.kind.clone(),
      name: item.name.clone(),
      type_hint: item.type_hint.clone(),
      count: 1,
    });
  }
  merged
}

fn schema_kind_shows_subtree(kind: &str) -> bool {
  matches!(kind, "atom-def" | "watch")
}

fn pick_richer_schema(current: Option<&str>, next: &str) -> String {
  match current {
    Some(cur) if cur.len() >= next.len() => cur.to_string(),
    _ => next.to_string(),
  }
}

fn schema_hint_for_atom_def(ns: &str, def: &str, context_ns: &str) -> Option<String> {
  let mut raw_candidates: Vec<String> = vec![];

  if let Some(code) = lookup_def_code(ns, def)
    && let Some(raw) = extract_defatom_init_from_code(&code, context_ns)
  {
    raw_candidates.push(raw);
  }

  if let Some(schema) = lookup_codegen_type_hint(ns, def)
    && let Some(hint) = format_schema_annotation(&schema, context_ns, MAX_SCHEMA_DEPTH)
  {
    raw_candidates.push(hint);
  }
  if let Some(hint) = format_schema_annotation(&lookup_def_schema(ns, def), context_ns, MAX_SCHEMA_DEPTH) {
    raw_candidates.push(hint);
  }

  raw_candidates
    .into_iter()
    .map(|raw| finalize_schema_hint(&raw, context_ns))
    .filter(|hint| is_informative_schema(hint))
    .max_by_key(|hint| hint.len())
}

fn enrich_atom_def_state(ns: &str, def: &str, state: &mut Vec<StateItem>) {
  let Some(schema) = lookup_atom_schema(ns, def) else {
    return;
  };
  if let Some(item) = state.iter_mut().find(|item| item.kind == "atom-def" && item.name == def) {
    item.type_hint = Some(pick_richer_schema(item.type_hint.as_deref(), &schema));
    return;
  }
  state.insert(
    0,
    StateItem {
      kind: "atom-def".into(),
      name: def.to_string(),
      type_hint: Some(schema),
    },
  );
}

fn extract_defatom_init_from_code(code: &Calcit, context_ns: &str) -> Option<String> {
  match code {
    Calcit::List(list) => {
      let head = list.first()?;
      let op = call_operator_name(head)?;
      if matches!(op.as_str(), "defatom" | "atom") {
        return list
          .get(2)
          .and_then(|init| summarize_init_expr(init, context_ns))
          .map(|raw| finalize_schema_hint(&raw, context_ns));
      }
      for item in list.iter() {
        if let Some(hint) = extract_defatom_init_from_code(item, context_ns) {
          return Some(hint);
        }
      }
      None
    }
    Calcit::Fn { info, .. } => {
      for expr in info.body.iter() {
        if let Some(hint) = extract_defatom_init_from_code(expr, context_ns) {
          return Some(hint);
        }
      }
      None
    }
    Calcit::Macro { info, .. } => {
      for expr in info.body.iter() {
        if let Some(hint) = extract_defatom_init_from_code(expr, context_ns) {
          return Some(hint);
        }
      }
      None
    }
    Calcit::Thunk(crate::calcit::CalcitThunk::Code { code, .. }) => extract_defatom_init_from_code(code, context_ns),
    _ => None,
  }
}

fn call_operator_name(head: &Calcit) -> Option<String> {
  match head {
    Calcit::Syntax(syntax, _) => Some(syntax.to_string()),
    Calcit::Proc(proc) => Some(proc.to_string()),
    Calcit::Registered(name) => Some(name.to_string()),
    Calcit::Import(import) => Some(import.def.to_string()),
    Calcit::Symbol { sym, .. } => Some(sym.to_string()),
    _ => None,
  }
}

fn summarize_init_expr(expr: &Calcit, current_ns: &str) -> Option<String> {
  match expr {
    Calcit::Number(n) => Some(format!(":number  init={n}")),
    Calcit::Nil => Some(":nil".into()),
    Calcit::Str(_) => Some(":string".into()),
    Calcit::Tag(tag) => Some(format!(":{tag}")),
    Calcit::Import(import) => Some(format!("{}/{}", import.ns, import.def)),
    Calcit::Symbol { sym, .. } if sym.contains('/') => Some(sym.to_string()),
    Calcit::Map(map) if map.is_empty() => Some(":map {}".into()),
    Calcit::List(list) if list.is_empty() => Some(":list []".into()),
    Calcit::List(list) => summarize_call_init(list, current_ns),
    _ => None,
  }
}

fn summarize_type_ref_expr(expr: &Calcit, current_ns: &str) -> Option<String> {
  match expr {
    Calcit::Import(import) => Some(format!("{}/{}", import.ns, import.def)),
    Calcit::Symbol { sym, .. } if sym.contains('/') => Some(sym.to_string()),
    _ => summarize_init_expr(expr, current_ns),
  }
}

fn expand_arrow_init(list: &crate::calcit::CalcitList, context_ns: &str) -> Option<String> {
  let base = list.get(1)?;
  let base_hint = summarize_type_ref_expr(base, context_ns)?;
  let mut overlays: Vec<String> = vec![];
  for idx in 2..list.len() {
    if let Some(item) = list.get(idx)
      && let Some(overlay) = extract_assoc_overlay(item, context_ns)
    {
      overlays.push(overlay);
    }
  }
  let base_expanded = finalize_schema_hint(&base_hint, context_ns);
  if overlays.is_empty() {
    Some(if base_expanded == base_hint {
      base_expanded
    } else {
      format!("{base_hint} = {base_expanded}")
    })
  } else {
    let base_label = if base_expanded == base_hint {
      base_hint
    } else {
      format!("{base_hint} = {base_expanded}")
    };
    Some(format!("{base_label} {{ {} }}", overlays.join(", ")))
  }
}

fn extract_assoc_overlay(expr: &Calcit, context_ns: &str) -> Option<String> {
  let Calcit::List(list) = expr else {
    return None;
  };
  let head = call_operator_name(list.first()?)?;
  if head != "assoc" {
    return None;
  }
  let key = extract_field_key(list.get(1)?)?;
  let val = list.get(2)?;
  let val_hint = summarize_type_ref_expr(val, context_ns)?;
  let val_expanded = finalize_schema_hint(&val_hint, context_ns);
  Some(format!(":{key} {val_expanded}"))
}

fn summarize_call_init(list: &crate::calcit::CalcitList, current_ns: &str) -> Option<String> {
  let head = list.first()?;

  if is_thread_first_head(head)
    && let Some(second) = list.get(1)
  {
    if is_arrow_head(second) {
      return list.get(2).and_then(|expr| summarize_init_expr(expr, current_ns));
    }
    return summarize_init_expr(second, current_ns);
  }

  if is_arrow_head(head) {
    return expand_arrow_init(list, current_ns);
  }

  if let Some((ns, def)) = resolve_def_call(head, current_ns) {
    return Some(format!("{ns}/{def}"));
  }
  if let Calcit::Symbol { sym, .. } = head {
    return Some(sym.to_string());
  }
  None
}

fn is_thread_first_head(head: &Calcit) -> bool {
  match head {
    Calcit::Symbol { sym, .. } => sym.as_ref() == "$",
    _ => false,
  }
}

fn is_arrow_head(head: &Calcit) -> bool {
  match head {
    Calcit::Symbol { sym, .. } => sym.as_ref() == "->",
    Calcit::Syntax(syntax, _) => syntax.to_string() == "->",
    Calcit::Import(import) => import.def.as_ref() == "->" || import.def.as_ref() == "$",
    _ => false,
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

fn infer_package_root(entry_ns: &str, ns_prefix: &Option<String>) -> Option<String> {
  if let Some(prefix) = ns_prefix {
    let root = prefix.strip_suffix('.').unwrap_or(prefix.as_str());
    if root.is_empty() {
      return None;
    }
    return Some(root.to_string());
  }
  crate::util::string::extract_pkg_from_ns(entry_ns.into()).map(|s| s.to_string())
}

fn shorten_fqn(fqn: &str, package_root: &str) -> String {
  let ns_prefix = format!("{package_root}.");
  let Some((ns, def)) = fqn.split_once('/') else {
    return fqn.to_string();
  };
  if ns.starts_with(&ns_prefix) {
    format!("{}/{}", &ns[ns_prefix.len()..], def)
  } else {
    fqn.to_string()
  }
}

struct SteDisplay<'a> {
  package_root: Option<&'a str>,
  color: SteColor,
}

struct SteColor {
  enabled: bool,
}

impl SteColor {
  fn new(enabled: bool) -> Self {
    if !enabled {
      colored::control::set_override(false);
    }
    Self { enabled }
  }

  fn plain(&self, text: &str) -> String {
    text.to_string()
  }

  fn paint<'a>(&self, text: &'a str, style: impl Fn(&'a str) -> colored::ColoredString) -> String {
    if self.enabled { style(text).to_string() } else { text.to_string() }
  }

  fn title(&self, text: &str) -> String {
    self.paint(text, |s| s.cyan().bold())
  }

  fn meta(&self, text: &str) -> String {
    self.paint(text, |s| s.dimmed())
  }

  fn connector(&self, text: &str) -> String {
    self.paint(text, |s| s.dimmed())
  }

  fn node_fqn(&self, text: &str, depth: usize) -> String {
    self.paint(text, |s| match depth {
      0 => s.green().bold(),
      1 => s.cyan(),
      2 => s.yellow(),
      _ => s.normal(),
    })
  }

  fn kind_label(&self, label: &str, node: &EffectsGraphNode) -> String {
    self.paint(label, |s| {
      let styled = if node.depth_exceeded {
        s.yellow()
      } else if node.circular {
        s.red()
      } else if node.source == "core" {
        s.cyan()
      } else if node.def.starts_with('*') {
        s.magenta()
      } else if node.effects.is_empty() {
        s.blue()
      } else {
        s.green()
      };
      styled.dimmed()
    })
  }

  fn schema(&self, text: &str) -> String {
    self.paint(text, |s| s.green().dimmed())
  }

  fn section_state(&self, text: &str) -> String {
    self.paint(text, |s| s.magenta().bold())
  }

  fn section_inputs(&self, text: &str) -> String {
    self.paint(text, |s| s.cyan().bold())
  }

  fn section_output(&self, text: &str) -> String {
    self.paint(text, |s| s.green().bold())
  }

  fn section_transform(&self, text: &str) -> String {
    self.paint(text, |s| s.blue().bold())
  }

  fn section_effects(&self, text: &str) -> String {
    self.paint(text, |s| s.yellow().bold())
  }

  fn state_kind(&self, text: &str) -> String {
    self.paint(text, |s| s.dimmed())
  }

  fn state_name(&self, text: &str) -> String {
    self.paint(text, |s| s.white().bold())
  }

  fn transform_arrow(&self, text: &str) -> String {
    self.paint(text, |s| s.blue())
  }

  fn effect_kind(&self, text: &str) -> String {
    self.paint(text, |s| s.yellow())
  }

  fn effect_target(&self, text: &str) -> String {
    self.plain(text)
  }

  fn collapsed(&self, text: &str) -> String {
    self.paint(text, |s| s.dimmed())
  }
}

impl<'a> SteDisplay<'a> {
  fn from_result(result: &'a EffectsGraphResult, color: bool) -> Self {
    SteDisplay {
      package_root: result.display.package_root.as_deref(),
      color: SteColor::new(color),
    }
  }

  fn fqn(&self, fqn: &str) -> String {
    match self.package_root {
      Some(root) => shorten_fqn(fqn, root),
      None => fqn.to_string(),
    }
  }
}

const SCHEMA_INLINE_MAX: usize = 72;

struct SchemaHintParts {
  label: Option<String>,
  segments: Vec<String>,
}

fn schema_should_embed(hint: &str) -> bool {
  hint.len() > SCHEMA_INLINE_MAX || (hint.contains('{') && hint.len() > 48)
}

fn split_schema_hint(hint: &str) -> SchemaHintParts {
  let hint = hint.trim();
  if let Some((label, rest)) = hint.split_once(" = ") {
    let segments = extract_brace_blocks(rest);
    if segments.is_empty() && !rest.trim().is_empty() {
      return SchemaHintParts {
        label: Some(label.trim().to_string()),
        segments: vec![rest.trim().to_string()],
      };
    }
    return SchemaHintParts {
      label: Some(label.trim().to_string()),
      segments,
    };
  }
  let segments = extract_brace_blocks(hint);
  if segments.is_empty() {
    SchemaHintParts {
      label: None,
      segments: vec![hint.to_string()],
    }
  } else {
    SchemaHintParts { label: None, segments }
  }
}

fn extract_brace_blocks(s: &str) -> Vec<String> {
  let mut blocks = Vec::new();
  let mut depth = 0usize;
  let mut start: Option<usize> = None;
  for (idx, ch) in s.char_indices() {
    match ch {
      '{' => {
        if depth == 0 {
          start = Some(idx);
        }
        depth += 1;
      }
      '}' => {
        if depth == 0 {
          continue;
        }
        depth -= 1;
        if depth == 0
          && let Some(from) = start
        {
          blocks.push(s[from..=idx].to_string());
          start = None;
        }
      }
      _ => {}
    }
  }
  blocks
}

fn parse_schema_segment(segment: &str) -> Option<Edn> {
  parse_schema_value(segment.trim())
}

fn parse_schema_value(raw: &str) -> Option<Edn> {
  let s = raw.trim();
  if s.is_empty() {
    return Some(Edn::Nil);
  }
  if s.starts_with('{') {
    return parse_schema_map(s);
  }
  if s.starts_with('[') {
    return parse_schema_list(s);
  }
  if s == "nil" {
    return Some(Edn::Nil);
  }
  if let Some(body) = s.strip_prefix(':') {
    let body = body.trim();
    if let Some(split_at) = body.find(char::is_whitespace) {
      let tag = &body[..split_at];
      let rest = body[split_at..].trim();
      let value = parse_schema_value(rest)?;
      let mut combo = EdnListView::default();
      combo.push(Edn::Tag(EdnTag::from(tag)));
      combo.push(value);
      return Some(combo.into());
    }
    return Some(Edn::Tag(EdnTag::from(body)));
  }
  Some(Edn::Symbol(s.into()))
}

fn parse_schema_map(s: &str) -> Option<Edn> {
  let inner = s.trim();
  if !inner.starts_with('{') || !inner.ends_with('}') {
    return None;
  }
  let body = inner[1..inner.len() - 1].trim();
  let mut map = EdnMapView::default();
  for field in split_top_level_commas(body) {
    let (key, value_raw) = split_field_key_value(field)?;
    let key_name = key.trim_start_matches(':');
    let value = parse_schema_value(value_raw)?;
    map.insert(Edn::Tag(EdnTag::from(key_name)), value);
  }
  Some(map.into())
}

fn parse_schema_list(s: &str) -> Option<Edn> {
  let inner = s.trim();
  if inner == "[]" {
    return Some(EdnListView::default().into());
  }
  if !inner.starts_with('[') || !inner.ends_with(']') {
    return None;
  }
  let body = inner[1..inner.len() - 1].trim();
  if body.is_empty() {
    return Some(EdnListView::default().into());
  }
  let mut list = EdnListView::default();
  for item in split_top_level_commas(body) {
    list.push(parse_schema_value(item)?);
  }
  Some(list.into())
}

fn split_field_key_value(field: &str) -> Option<(&str, &str)> {
  let field = field.trim();
  if !field.starts_with(':') {
    return None;
  }
  let rest = field[1..].trim_start();
  let key_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
  let key = &field[..1 + key_end];
  let value = rest[key_end..].trim();
  Some((key, value))
}

fn split_top_level_commas(s: &str) -> Vec<&str> {
  let mut parts = Vec::new();
  let mut start = 0usize;
  let mut brace = 0usize;
  let mut bracket = 0usize;
  for (idx, ch) in s.char_indices() {
    match ch {
      '{' => brace += 1,
      '}' => brace = brace.saturating_sub(1),
      '[' => bracket += 1,
      ']' => bracket = bracket.saturating_sub(1),
      ',' if brace == 0 && bracket == 0 => {
        let part = s[start..idx].trim();
        if !part.is_empty() {
          parts.push(part);
        }
        start = idx + 1;
      }
      _ => {}
    }
  }
  let part = s[start..].trim();
  if !part.is_empty() {
    parts.push(part);
  }
  parts
}

fn format_schema_edn_lines(edn: &Edn) -> Vec<String> {
  cirru_edn::format(edn, true)
    .map(|text| {
      text
        .lines()
        .map(|line| line.trim_end().to_string())
        .filter(|line| !line.trim().is_empty())
        .collect()
    })
    .unwrap_or_else(|_| vec![edn.to_string()])
}

fn render_schema_subtree(hint: &str, prefix: &str, display: &SteDisplay<'_>, out: &mut String) {
  let parts = split_schema_hint(hint);
  out.push_str(&format!(
    "{}{}\n",
    display.color.connector(prefix),
    display.color.schema("└── schema")
  ));
  let inner = format!("{prefix}    ");

  if let Some(label) = parts.label {
    out.push_str(&format!(
      "{}{}\n",
      display.color.connector(&inner),
      display.color.schema(&format!("{label} ="))
    ));
  }

  let segments = if parts.segments.is_empty() {
    vec![hint.to_string()]
  } else {
    parts.segments
  };

  for segment in segments {
    if let Some(edn) = parse_schema_segment(&segment) {
      for line in format_schema_edn_lines(&edn) {
        out.push_str(&format!("{}{}\n", display.color.connector(&inner), display.color.schema(&line)));
      }
    } else {
      out.push_str(&format!("{}{}\n", display.color.connector(&inner), display.color.schema(&segment)));
    }
  }
}

fn inline_schema_suffix(hint: &str, display: &SteDisplay<'_>) -> String {
  if schema_should_embed(hint) {
    return String::new();
  }
  format!("  {}", display.color.schema(&format!("schema: {hint}")))
}

fn node_atom_schema_hint(node: &EffectsGraphNode) -> Option<String> {
  if !node.def.starts_with('*') {
    return None;
  }
  node
    .state
    .iter()
    .find(|item| item.kind == "atom-def")
    .and_then(|item| item.type_hint.clone())
    .or_else(|| lookup_atom_schema(&node.ns, &node.def))
}

/// Format entry point for CLI banners and headers.
pub fn format_entry_label(entry_ns: &str, entry_def: &str, ns_prefix: Option<&str>) -> String {
  let fqn = format!("{entry_ns}/{entry_def}");
  match infer_package_root(entry_ns, &ns_prefix.map(|s| s.to_string())) {
    Some(root) => shorten_fqn(&fqn, &root),
    None => fqn,
  }
}

/// Render per-function STE decomposition tree.
pub fn format_as_ste_tree(result: &EffectsGraphResult, color: bool) -> String {
  let display = SteDisplay::from_result(result, color);
  let mut out = String::new();
  out.push_str(&format!(
    "{}\n\n",
    display.color.title(&format!("# Effects Graph: `{}`", display.fqn(&result.entry)))
  ));

  if let Some(root) = display.package_root {
    out.push_str(&format!("{}\n", display.color.meta(&format!("Package: `{root}.*`"))));
  }

  let depth_limited = count_depth_limited(&result.tree);
  if depth_limited > 0 {
    out.push_str(&format!(
      "{}\n\n",
      display.color.meta(&format!(
        "Max depth: {}  ({} nodes truncated; rerun with larger --max-depth to expand)",
        result.display.max_depth_limit, depth_limited
      ))
    ));
  } else if result.display.max_depth_limit > 0 {
    out.push_str(&format!(
      "{}\n\n",
      display.color.meta(&format!("Max depth: {}", result.display.max_depth_limit))
    ));
  } else if display.package_root.is_some() {
    out.push('\n');
  }

  render_ste_node(&result.tree, &mut out, "", true, 0, &display);
  out
}

fn count_depth_limited(node: &EffectsGraphNode) -> usize {
  let mut count = if node.depth_exceeded { 1 } else { 0 };
  for child in &node.children {
    count += count_depth_limited(child);
  }
  count
}

fn render_ste_node(node: &EffectsGraphNode, out: &mut String, prefix: &str, is_last: bool, depth: usize, display: &SteDisplay<'_>) {
  if node.seen {
    return;
  }

  let connector = if is_last { "└── " } else { "├── " };
  let (kind_label, is_collapsed) = ste_kind_label(node);
  let summary = node_transform_summary(node);
  let fqn = display.fqn(&node.fqn);

  out.push_str(&format!(
    "{}{}{}  {}{}\n",
    display.color.connector(prefix),
    display.color.connector(connector),
    display.color.node_fqn(&fqn, depth),
    display.color.kind_label(kind_label, node),
    node_schema_suffix(node, display)
  ));

  let child_prefix = format!("{prefix}{}   ", if is_last { " " } else { "│" });

  if is_collapsed {
    if !summary.is_empty() {
      out.push_str(&format!(
        "{}{}\n",
        display.color.connector(&child_prefix),
        display.color.collapsed(&format!("({summary})"))
      ));
    }
    return;
  }

  render_ste_inputs(node, out, &child_prefix, display);
  render_ste_output(node, out, &child_prefix, display);
  render_ste_state(node, out, &child_prefix, display);
  render_ste_transform(node, out, &child_prefix, display);
  render_ste_effects(node, out, &child_prefix, display);

  if !node.children.is_empty() {
    out.push_str(&format!(
      "{}{}\n",
      display.color.connector(&child_prefix),
      display.color.connector("│")
    ));
  }
  for (idx, child) in node.children.iter().enumerate() {
    let is_last_child = idx + 1 == node.children.len();
    render_ste_node(child, out, &child_prefix, is_last_child, depth + 1, display);
  }
}

fn node_schema_suffix(node: &EffectsGraphNode, display: &SteDisplay<'_>) -> String {
  node_atom_schema_hint(node)
    .as_deref()
    .map(|hint| inline_schema_suffix(hint, display))
    .unwrap_or_default()
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

fn render_ste_inputs(node: &EffectsGraphNode, out: &mut String, prefix: &str, display: &SteDisplay<'_>) {
  let params: Vec<&StateItem> = node.state.iter().filter(|item| item.kind == "param").collect();
  if params.is_empty() {
    return;
  }
  let entries = merge_state_items(params.into_iter().map(|item| (state_kind_label(&item.kind), item)).collect());
  render_ste_entry_section(
    "├── Inputs",
    |color, text| color.section_inputs(text),
    &entries,
    prefix,
    display,
    out,
  );
}

fn render_ste_output(node: &EffectsGraphNode, out: &mut String, prefix: &str, display: &SteDisplay<'_>) {
  let returns: Vec<&StateItem> = node.state.iter().filter(|item| item.kind == "return").collect();
  if returns.is_empty() {
    return;
  }
  let entries = merge_state_items(returns.into_iter().map(|item| (state_kind_label(&item.kind), item)).collect());
  render_ste_entry_section(
    "├── Output",
    |color, text| color.section_output(text),
    &entries,
    prefix,
    display,
    out,
  );
}

fn render_ste_state(node: &EffectsGraphNode, out: &mut String, prefix: &str, display: &SteDisplay<'_>) {
  let mut raw: Vec<(&str, &StateItem)> = vec![];
  for item in &node.state {
    match item.kind.as_str() {
      "param" | "return" => {}
      "atom-def" | "atom" | "atom-read" | "atom-write" => {
        raw.push((state_kind_label(&item.kind), item));
      }
      "watch" => raw.push(("watch", item)),
      _ => raw.push((state_kind_label(&item.kind), item)),
    }
  }
  if raw.is_empty() {
    return;
  }
  let entries = merge_state_items(raw);
  render_ste_entry_section("├── State", |color, text| color.section_state(text), &entries, prefix, display, out);
}

fn render_ste_entry_section(
  header: &str,
  header_style: impl Fn(&SteColor, &str) -> String,
  entries: &[MergedStateItem],
  prefix: &str,
  display: &SteDisplay<'_>,
  out: &mut String,
) {
  out.push_str(&format!(
    "{}{}\n",
    display.color.connector(prefix),
    header_style(&display.color, header)
  ));
  let item_prefix = format!("{prefix}│   ");
  for (idx, entry) in entries.iter().enumerate() {
    let is_last = idx + 1 == entries.len();
    let conn = if is_last { "└── " } else { "├── " };
    let line = ste_merged_state_line(entry, display);
    out.push_str(&format!(
      "{}{}{}\n",
      display.color.connector(&item_prefix),
      display.color.connector(conn),
      line
    ));
    if schema_kind_shows_subtree(&entry.kind)
      && let Some(hint) = entry.type_hint.as_deref().filter(|hint| schema_should_embed(hint))
    {
      let sub_prefix = format!("{item_prefix}{}", if is_last { "    " } else { "│   " });
      render_schema_subtree(hint, &sub_prefix, display, out);
    }
  }
}

fn ste_merged_state_line(entry: &MergedStateItem, display: &SteDisplay<'_>) -> String {
  let kind = if entry.kind_label.is_empty() {
    state_kind_label(&entry.kind)
  } else {
    entry.kind_label.as_str()
  };

  if entry.kind == "return" {
    let value = entry.type_hint.as_deref().unwrap_or(&entry.name);
    return format!(
      "{} {}",
      display.color.state_kind(&format!("{kind:<8}")),
      display.color.state_name(value)
    );
  }

  if entry.kind == "param" {
    let type_part = entry
      .type_hint
      .as_deref()
      .map(|hint| format!("  {}", display.color.schema(hint)))
      .unwrap_or_default();
    return format!(
      "{} {}{type_part}",
      display.color.state_kind(&format!("{kind:<8}")),
      display.color.state_name(&entry.name)
    );
  }

  let mut name = display.color.state_name(&entry.name);
  if entry.kind == "atom-write" {
    name.push_str(&display.color.meta(" (write)"));
  } else if entry.kind == "atom-read" {
    name.push_str(&display.color.meta(" (read)"));
  } else if entry.kind == "local-write" {
    name.push_str(&display.color.meta(" (set!)"));
  }
  if entry.count > 1 {
    name.push_str(&display.color.meta(&format!(" (×{})", entry.count)));
  }

  let schema = entry
    .type_hint
    .as_deref()
    .filter(|_| schema_kind_shows_subtree(&entry.kind))
    .map(|hint| inline_schema_suffix(hint, display))
    .unwrap_or_default();

  format!("{} {}{schema}", display.color.state_kind(&format!("{kind:<8}")), name)
}

fn render_ste_transform(node: &EffectsGraphNode, out: &mut String, prefix: &str, display: &SteDisplay<'_>) {
  let has_control = !node.transform.control.is_empty();
  let has_calls = !node.transform.calls.is_empty();
  let summary = if node.transform.summary.is_empty() || node.transform.summary == "transform" {
    String::new()
  } else {
    format!("  ({})", node.transform.summary)
  };

  if !has_control && !has_calls {
    out.push_str(&format!(
      "{}{}{}\n",
      display.color.connector(prefix),
      display.color.section_transform("├── Transform"),
      display.color.meta(&summary)
    ));
    out.push_str(&format!(
      "{}{}{}\n",
      display.color.connector(prefix),
      display.color.connector("│   └── "),
      display.color.collapsed("(no calls)")
    ));
    return;
  }

  out.push_str(&format!(
    "{}{}{}\n",
    display.color.connector(prefix),
    display.color.section_transform("├── Transform"),
    display.color.meta(&summary)
  ));
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
    lines.push(display.color.meta(&format!("control: {}", control_summary.join(", "))));
  }

  for call in &node.transform.calls {
    lines.push(format!(
      "{} {}",
      display.color.transform_arrow("→"),
      display.color.node_fqn(&display.fqn(call), 1)
    ));
  }

  for (idx, line) in lines.iter().enumerate() {
    let conn = if idx + 1 == lines.len() { "└── " } else { "├── " };
    out.push_str(&format!(
      "{}{}{}\n",
      display.color.connector(&item_prefix),
      display.color.connector(conn),
      line
    ));
  }
}

fn render_ste_effects(node: &EffectsGraphNode, out: &mut String, prefix: &str, display: &SteDisplay<'_>) {
  if node.effects.is_empty() {
    out.push_str(&format!(
      "{}{}\n",
      display.color.connector(prefix),
      display.color.section_effects("└── Effects")
    ));
    out.push_str(&format!(
      "{}{}{}\n",
      display.color.connector(prefix),
      display.color.connector("    └── "),
      display.color.collapsed("(none — pure transform)")
    ));
    return;
  }

  out.push_str(&format!(
    "{}{}\n",
    display.color.connector(prefix),
    display.color.section_effects("└── Effects")
  ));
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
      display.color.meta(&format!(" (×{})", effect.count))
    } else {
      String::new()
    };
    out.push_str(&format!(
      "{}{}{} {}{}\n",
      display.color.connector(&item_prefix),
      display.color.connector(conn),
      display.color.effect_kind(&format!("{:<14}", effect.kind)),
      display.color.effect_target(&effect.target),
      count_suffix
    ));
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
  fn summarize_number_init_expr() {
    let init = Calcit::Number(0.0);
    assert_eq!(summarize_init_expr(&init, "app.main").as_deref(), Some(":number  init=0"));
  }

  #[test]
  fn expand_store_schema_when_msg_buffer_available() {
    let path = std::path::Path::new("/Users/jon.chen/repo/termina/msg-buffer/calcit.cirru");
    if !path.exists() {
      return;
    }
    let content = std::fs::read_to_string(path).expect("read msg-buffer snapshot");
    let data = cirru_edn::parse(&content).expect("parse snapshot");
    let mut snapshot = crate::snapshot::load_snapshot_data(&data, path.to_string_lossy().as_ref()).expect("load snapshot");
    let core = crate::load_core_snapshot().expect("load core");
    for (k, v) in core.files {
      snapshot.files.insert(k.to_owned(), v.to_owned());
    }
    {
      let mut prgm = crate::program::PROGRAM_CODE_DATA.write().expect("program data");
      *prgm = crate::program::extract_program_data(&snapshot).expect("extract program");
    }

    let resolved = resolve_type_ref_path("schema/store", "app.main").expect("resolve schema/store");
    assert_eq!(resolved.0, "app.schema");
    assert_eq!(resolved.1, "store");

    let code = lookup_def_code(&resolved.0, &resolved.1).expect("lookup store code");
    let template = find_best_struct_template(&code);
    assert!(template.is_some(), "record template not found in code: {code:?}");

    let expanded = expand_type_ref_path("schema/store", "app.main", MAX_SCHEMA_DEPTH).expect("expand schema/store");
    assert!(expanded.contains(":states"), "expected store fields, got: {expanded}");
  }

  #[test]
  fn schema_hint_parses_segment_to_edn() {
    let hint = "{ :records :list [], :base :nil, :stopped? :bool }";
    let edn = parse_schema_segment(hint).expect("parse record");
    let lines = format_schema_edn_lines(&edn);
    let joined = lines.join("\n");
    assert!(joined.contains(":records"));
    assert!(joined.contains(":list"));
    assert!(joined.contains(":base"));
  }

  #[test]
  fn schema_edn_preserves_indentation() {
    let hint = "{ :base { :cursor :list [], :count :number }, :store :nil }";
    let edn = parse_schema_segment(hint).expect("parse nested record");
    let raw = cirru_edn::format(&edn, true).expect("format edn");
    assert!(
      raw.lines().any(|line| line.starts_with(' ') || line.starts_with('\t')),
      "expected cirru edn indent, got:\n{raw}"
    );

    let lines = format_schema_edn_lines(&edn);
    assert!(
      lines.iter().any(|line| line.starts_with(' ') || line.starts_with('\t')),
      "expected preserved indent, got:\n{}",
      lines.join("\n")
    );
  }

  #[test]
  fn long_schema_renders_as_indented_subtree() {
    let hint = "reel-schema/reel = { :records :list [], :base :nil, :store :nil, :pointer :nil, :stopped? :bool, :display? :bool, :merged? :bool } { :base { :states { :cursor :list [] }, :sessions :list [] }, :store { :states { :cursor :list [] } } }";
    assert!(schema_should_embed(hint));

    let result = EffectsGraphResult {
      entry: "app.main/main!".into(),
      tree: EffectsGraphNode {
        ns: "app.main".into(),
        def: "main!".into(),
        fqn: "app.main/main!".into(),
        doc: None,
        source: "project".into(),
        state: vec![StateItem {
          kind: "watch".into(),
          name: "*reel".into(),
          type_hint: Some(hint.to_string()),
        }],
        transform: TransformInfo::default(),
        effects: vec![],
        children: vec![],
        circular: false,
        seen: false,
        depth_exceeded: false,
      },
      stats: EffectsGraphStats {
        reachable_count: 1,
        effect_sites: 0,
        state_items: 1,
        max_depth: 0,
        subgraph_count: 1,
      },
      display: EffectsGraphDisplayMeta {
        max_depth_limit: 2,
        detail: "summary".into(),
        include_core: false,
        ns_prefix: None,
        package_root: Some("app".into()),
      },
    };

    let text = format_as_ste_tree(&result, false);
    assert!(text.contains("watch    *reel\n"));
    assert!(!text.contains("schema: reel-schema"));
    assert!(text.contains("└── schema"));
    assert!(text.contains("reel-schema/reel ="));
    assert!(text.contains(":records"));
    assert!(text.contains(":base"));
  }

  #[test]
  fn duplicate_atom_reads_merge_in_display() {
    let result = EffectsGraphResult {
      entry: "app.main/render!".into(),
      tree: EffectsGraphNode {
        ns: "app.main".into(),
        def: "render!".into(),
        fqn: "app.main/render!".into(),
        doc: None,
        source: "project".into(),
        state: vec![
          StateItem {
            kind: "atom-read".into(),
            name: "*reel".into(),
            type_hint: None,
          },
          StateItem {
            kind: "atom-read".into(),
            name: "*reel".into(),
            type_hint: None,
          },
        ],
        transform: TransformInfo::default(),
        effects: vec![],
        children: vec![],
        circular: false,
        seen: false,
        depth_exceeded: false,
      },
      stats: EffectsGraphStats {
        reachable_count: 1,
        effect_sites: 0,
        state_items: 2,
        max_depth: 0,
        subgraph_count: 1,
      },
      display: EffectsGraphDisplayMeta {
        max_depth_limit: 2,
        detail: "summary".into(),
        include_core: false,
        ns_prefix: None,
        package_root: Some("app".into()),
      },
    };

    let text = format_as_ste_tree(&result, false);
    assert_eq!(text.matches("atom     *reel (read)").count(), 1);
    assert!(text.contains("(×2)"));
  }

  #[test]
  fn ste_tree_shows_inputs_and_output_sections() {
    let result = EffectsGraphResult {
      entry: "app.main/main!".into(),
      tree: EffectsGraphNode {
        ns: "app.main".into(),
        def: "dispatch!".into(),
        fqn: "app.main/dispatch!".into(),
        doc: None,
        source: "project".into(),
        state: vec![
          StateItem {
            kind: "param".into(),
            name: "event".into(),
            type_hint: Some(":map".into()),
          },
          StateItem {
            kind: "return".into(),
            name: "return".into(),
            type_hint: Some(":nil".into()),
          },
        ],
        transform: TransformInfo::default(),
        effects: vec![],
        children: vec![],
        circular: false,
        seen: false,
        depth_exceeded: false,
      },
      stats: EffectsGraphStats {
        reachable_count: 1,
        effect_sites: 0,
        state_items: 2,
        max_depth: 0,
        subgraph_count: 1,
      },
      display: EffectsGraphDisplayMeta {
        max_depth_limit: 2,
        detail: "summary".into(),
        include_core: false,
        ns_prefix: None,
        package_root: Some("app".into()),
      },
    };

    let text = format_as_ste_tree(&result, false);
    assert!(text.contains("├── Inputs"));
    assert!(text.contains("in       event"));
    assert!(text.contains("├── Output"));
    assert!(text.contains("out      :nil"));
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
        package_root: Some("app".into()),
      },
    };

    let text = format_as_ste_tree(&result, false);
    assert!(text.contains("# Effects Graph"));
    assert!(text.contains("main/main!"));
    assert!(!text.contains("app.main/main!"));
    assert!(text.contains("console"));
    assert!(text.contains("println"));
    assert!(text.contains("[program]"));
  }
}
