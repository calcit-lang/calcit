//! Query subcommand handlers
//!
//! Handles: calcit query ns, defs, def, at, peek, examples, find, usages, pkg, config, error, modules

use super::chunk_display::{ChunkDisplayOptions, ChunkedDisplay, maybe_chunk_node};
use super::common::{
  cirru_to_json_value, deps_path_for_snapshot, emit_cli_output, format_path, parse_path, print_cli_warning_block,
  resolve_definition_lookup,
};
use super::cursor::{
  CursorLastQuery, load_cursor_last_query, resolve_active_cursor_reference, resolve_cursor_path_argument,
  resolve_cursor_target_argument, set_cursor_from_query_match,
};
use super::tips::{TipPriority, Tips, command_guidance_enabled};
use calcit::CalcitTypeAnnotation;
use calcit::calcit::{Calcit, CalcitFnTypeAnnotation, DYNAMIC_TYPE, LocatedWarning};
use calcit::call_stack::CallStackList;
use calcit::call_tree::{CallTreeAnalyzer, CallTreeConfig};
use calcit::cli_args::{
  QueryAnchorsCommand, QueryCommand, QueryContextCommand, QueryDefCommand, QueryDefsCommand, QueryHostProcsCommand, QueryPathCommand,
  QuerySubcommand, QueryTypeAtCommand, QueryTypeCommand,
};
use calcit::data::cirru::code_to_calcit;
use calcit::data::edn::format_edn_display;
use calcit::load_core_snapshot;
use calcit::project_state::{self, ERROR_STATE_FILE};
use calcit::snapshot;
use calcit::util::string::strip_shebang;
use calcit::{program, runner};
use cirru_edn::EdnTag;
use cirru_parser::Cirru;
use colored::Colorize;
use md5::{Digest, Md5};
use serde::Serialize;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::edit::navigate_to_path;

/// Type alias for search results: (namespace, definition, matches)
type SearchResults = Vec<(String, String, Vec<(Vec<usize>, Cirru)>)>;

/// Type alias for reference results: (namespace, definition, context, coordinate-path, source-label)
type RefResults = Vec<(String, String, String, Vec<Vec<usize>>, &'static str)>;

struct SearchCommonOpts<'a> {
  filter: Option<&'a str>,
  loose: bool,
  regex: bool,
  max_depth: usize,
  entry: Option<&'a str>,
  detail_offset: usize,
  parent_path: bool,
  format: QueryRenderFormat,
  set_cursor: Option<usize>,
  compact_output: bool,
}

const DETAILED_RESULTS_WINDOW: usize = 3;

struct SpecialBuiltinQueryMeta {
  doc: &'static str,
  schema: Arc<CalcitTypeAnnotation>,
  examples: Vec<Cirru>,
  expr_preview: &'static str,
  cirru_note: &'static str,
  semantic_tags: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueryRenderFormat {
  Human,
  Json,
}

fn parse_query_render_format(raw: &str) -> Result<QueryRenderFormat, String> {
  match raw {
    "human" | "text" => Ok(QueryRenderFormat::Human),
    "json" => Ok(QueryRenderFormat::Json),
    other => Err(format!("Unknown query output format `{other}`. Expected `human` or `json`.")),
  }
}

fn semantic_revision(parts: &[&str]) -> String {
  let mut hasher = Md5::new();
  for part in parts {
    hasher.update((part.len() as u64).to_le_bytes());
    hasher.update(part.as_bytes());
  }
  format!("md5:{}", hex::encode(hasher.finalize()))
}

#[derive(Debug, Serialize)]
struct SemanticQueryEnvelope<T> {
  schema_version: u32,
  command: &'static str,
  revision: String,
  data: T,
  diagnostics: Vec<ContextDiagnostic>,
  next: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ContextMethod {
  name: String,
  origin: String,
}

#[derive(Debug, Serialize)]
struct TypeQueryData {
  target: String,
  canonical_type: String,
  resolved_from: &'static str,
  methods: Option<Vec<ContextMethod>>,
}

#[derive(Debug, Clone, Serialize)]
struct TypeAtEvidence {
  kind: String,
  detail: String,
  path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct TypeAtBinding {
  name: String,
  r#type: String,
  path: Option<String>,
}

#[derive(Debug, Serialize)]
struct TypeAtData {
  id: String,
  path: String,
  expression: String,
  tree: serde_json::Value,
  inferred_type: Option<String>,
  expected_type: Option<String>,
  expected_from: Option<String>,
  confidence: &'static str,
  dynamic_intent: Option<&'static str>,
  evidence: Vec<TypeAtEvidence>,
  bindings: Vec<TypeAtBinding>,
  bindings_complete: bool,
  static_methods: Option<Vec<ContextMethod>>,
}

#[derive(Debug, Clone, Serialize)]
struct ContextCollection<T> {
  total: usize,
  returned: usize,
  truncated: bool,
  items: Vec<T>,
}

impl<T> ContextCollection<T> {
  fn new(total: usize, items: Vec<T>) -> Self {
    let returned = items.len();
    Self {
      total,
      returned,
      truncated: returned < total,
      items,
    }
  }
}

#[derive(Debug, Clone, Serialize)]
struct ContextCode {
  root: &'static str,
  nodes: usize,
  cirru: String,
  tree: Option<serde_json::Value>,
  truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ContextExample {
  index: usize,
  cirru: String,
  tree: Option<serde_json::Value>,
  truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ContextTest {
  name: String,
  tags: Vec<String>,
  cirru: String,
  tree: Option<serde_json::Value>,
  truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ContextDependency {
  id: String,
  source: String,
}

#[derive(Debug, Clone, Serialize)]
struct ContextUsage {
  id: String,
  source: String,
  area: String,
  paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ContextDocLink {
  id: String,
  path: String,
  title: Option<String>,
  summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ContextDiagnostic {
  code: String,
  phase: &'static str,
  severity: &'static str,
  message: String,
  path: Option<String>,
  intent: Option<String>,
}

#[derive(Debug, Serialize)]
struct DefinitionContextData {
  id: String,
  uri: String,
  source: String,
  kind: String,
  coverage: String,
  doc: Option<String>,
  doc_truncated: bool,
  tags: Vec<String>,
  schema: Option<String>,
  features: Vec<String>,
  code: ContextCode,
  examples: ContextCollection<ContextExample>,
  tests: ContextCollection<ContextTest>,
  dependencies: ContextCollection<ContextDependency>,
  usages: ContextCollection<ContextUsage>,
  docs: ContextCollection<ContextDocLink>,
  static_methods: Option<ContextCollection<ContextMethod>>,
}

fn special_builtin_dynamic_fn(arg_types: Vec<Arc<CalcitTypeAnnotation>>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::from_function_parts(arg_types, DYNAMIC_TYPE.clone()))
}

fn parse_special_builtin_examples(snippets: &[&str]) -> Result<Vec<Cirru>, String> {
  let mut examples = Vec::with_capacity(snippets.len());
  for snippet in snippets {
    let parsed = cirru_parser::parse(snippet).map_err(|e| format!("Failed to parse builtin example `{snippet}`: {e}"))?;
    let Some(example) = parsed.into_iter().next() else {
      return Err(format!("Builtin example is empty: {snippet}"));
    };
    examples.push(example);
  }
  Ok(examples)
}

fn lookup_special_builtin_query_meta(namespace: &str, definition: &str) -> Result<Option<SpecialBuiltinQueryMeta>, String> {
  if namespace != calcit::calcit::CORE_NS {
    return Ok(None);
  }

  let meta = match definition {
    "to-calcit-data" => Some(SpecialBuiltinQueryMeta {
      doc: "convert JS arrays and plain objects into Calcit data recursively",
      schema: special_builtin_dynamic_fn(vec![DYNAMIC_TYPE.clone()]),
      examples: parse_special_builtin_examples(&[
        "to-calcit-data $ js-array 1 ([] 2 3) (:: :quote $ [] 'a 'b)",
        "to-calcit-data $ &js-object |a 1 |:b 2 :c ([] 3 4)",
      ])?,
      expr_preview: "builtin proc to-calcit-data (JS interop helper)",
      cirru_note: "(builtin proc; runtime helper without snapshot source)",
      semantic_tags: &["js-ffi"],
    }),
    "to-js-data" => Some(SpecialBuiltinQueryMeta {
      doc: "convert Calcit data into JS-compatible data recursively; JS FFI keeps value and options dynamically typed",
      schema: special_builtin_dynamic_fn(vec![DYNAMIC_TYPE.clone(), DYNAMIC_TYPE.clone()]),
      examples: parse_special_builtin_examples(&[
        "to-js-data $ [] 1 2 3",
        "to-js-data $ &{} |a 1 :b ([] 2 3)",
        "to-js-data ([] 1 2 3) $ {} (:js-array true)",
      ])?,
      expr_preview: "builtin proc to-js-data (JS interop helper)",
      cirru_note: "(builtin proc; runtime helper without snapshot source)",
      semantic_tags: &["js-ffi"],
    }),
    "to-cirru-edn" => Some(SpecialBuiltinQueryMeta {
      doc: "convert Calcit data into Cirru EDN data",
      schema: special_builtin_dynamic_fn(vec![DYNAMIC_TYPE.clone()]),
      examples: vec![],
      expr_preview: "builtin proc to-cirru-edn (data conversion helper)",
      cirru_note: "(builtin proc; runtime helper without snapshot source)",
      semantic_tags: &[],
    }),
    "extract-cirru-edn" => Some(SpecialBuiltinQueryMeta {
      doc: "extract Cirru EDN data into regular Calcit values when possible",
      schema: special_builtin_dynamic_fn(vec![DYNAMIC_TYPE.clone()]),
      examples: vec![],
      expr_preview: "builtin proc extract-cirru-edn (data conversion helper)",
      cirru_note: "(builtin proc; runtime helper without snapshot source)",
      semantic_tags: &[],
    }),
    "js-array" => Some(SpecialBuiltinQueryMeta {
      doc: "build a JS array value in JS interop contexts",
      schema: special_builtin_dynamic_fn(vec![Arc::new(CalcitTypeAnnotation::Variadic(DYNAMIC_TYPE.clone()))]),
      examples: vec![],
      expr_preview: "builtin proc js-array (JS interop helper)",
      cirru_note: "(builtin proc; runtime helper without snapshot source)",
      semantic_tags: &["js-ffi"],
    }),
    "&js-object" => Some(SpecialBuiltinQueryMeta {
      doc: "build a plain JS object value in JS interop contexts",
      schema: special_builtin_dynamic_fn(vec![Arc::new(CalcitTypeAnnotation::Variadic(DYNAMIC_TYPE.clone()))]),
      examples: vec![],
      expr_preview: "builtin proc &js-object (JS interop helper)",
      cirru_note: "(builtin proc; runtime helper without snapshot source)",
      semantic_tags: &["js-ffi"],
    }),
    _ => None,
  };

  Ok(meta)
}

fn query_schema_cirru(annotation: &CalcitTypeAnnotation, wrapped: bool) -> Result<Option<Cirru>, String> {
  let schema_edn = match annotation {
    CalcitTypeAnnotation::Dynamic => return Ok(None),
    CalcitTypeAnnotation::Fn(fn_annot) if wrapped => fn_annot.to_wrapped_schema_edn(),
    CalcitTypeAnnotation::Fn(fn_annot) => fn_annot.to_schema_edn(),
    other => other.to_type_edn(),
  };
  snapshot::schema_edn_to_cirru(&schema_edn).map(|cirru| {
    let normalized = match cirru {
      Cirru::List(items) if items.len() == 2 && matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "do") => {
        items[1].clone()
      }
      other => other,
    };
    Some(normalized)
  })
}

fn normalize_single_schema_display(text: String) -> String {
  let trimmed = text.trim();
  trimmed.strip_prefix("do ").unwrap_or(trimmed).to_owned()
}

fn format_query_schema_oneline(cirru: &Cirru) -> Result<String, String> {
  match cirru {
    Cirru::Leaf(value) => Ok(value.to_string()),
    Cirru::List(_) => cirru.format_one_liner().map(normalize_single_schema_display),
  }
}

fn format_query_schema(annotation: &CalcitTypeAnnotation, wrapped: bool) -> String {
  match query_schema_cirru(annotation, wrapped) {
    Ok(Some(Cirru::Leaf(value))) => value.to_string(),
    Ok(Some(cirru @ Cirru::List(_))) => cirru_parser::format(std::slice::from_ref(&cirru), true.into())
      .map(normalize_single_schema_display)
      .unwrap_or_else(|_| "(failed to format)".to_string()),
    Ok(None) => "(none)".to_string(),
    Err(error) => format!("(schema error: {error})"),
  }
}

fn format_schema_query_json(id: &str, source: &str, annotation: &CalcitTypeAnnotation, revision: String) -> Result<String, String> {
  let schema = query_schema_cirru(annotation, true)?;
  let canonical_schema = schema.as_ref().map(format_query_schema_oneline).transpose()?;
  let tree = schema.as_ref().map(cirru_to_json_value);
  let envelope = serde_json::json!({
    "schema_version": 1,
    "command": "query.schema",
    "revision": revision,
    "data": {
      "id": id,
      "source": source,
      "canonical_schema": canonical_schema,
      "tree": tree,
    },
    "diagnostics": [],
  });
  serde_json::to_string_pretty(&envelope).map_err(|error| format!("Failed to encode schema query JSON: {error}"))
}

fn detailed_window(detail_offset: usize, total: usize) -> (usize, usize) {
  if total == 0 {
    return (0, 0);
  }
  let start = detail_offset.min(total.saturating_sub(1));
  let end = (start + DETAILED_RESULTS_WINDOW).min(total);
  (start, end)
}

fn print_detail_window_hint(total: usize, detail_offset: usize, subject: &str) {
  if total > DETAILED_RESULTS_WINDOW {
    let (start, end) = detailed_window(detail_offset, total);
    println!(
      "{}",
      format!("Detail window for {subject}: [{start}, {end}) (detail-offset={detail_offset}), other entries are compressed.").dimmed()
    );
  }
}

fn in_detail_window(index: usize, total: usize, detail_offset: usize) -> bool {
  if total <= DETAILED_RESULTS_WINDOW {
    return true;
  }
  let (start, end) = detailed_window(detail_offset, total);
  index >= start && index < end
}

fn preview_node_oneline(node: &Cirru, max_len: usize) -> (String, bool) {
  let text = match node {
    Cirru::Leaf(s) => s.to_string(),
    _ => node.format_one_liner().unwrap_or_default(),
  };
  if text.is_empty() {
    return ("(matched)".to_string(), false);
  }
  if text.len() > max_len {
    (text[..max_len].to_string(), true)
  } else {
    (text, false)
  }
}

fn is_token_delimiter(ch: Option<char>) -> bool {
  match ch {
    None => true,
    Some(c) => c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | '$' | ','),
  }
}

fn highlight_target_text(text: &str, target: Option<&str>, loose: bool) -> String {
  let Some(target) = target else {
    return text.to_string();
  };
  if target.is_empty() || !text.contains(target) {
    return text.to_string();
  }

  if loose {
    return text.replace(target, &format!("{}", target.bright_yellow().bold()));
  }

  let mut highlighted = String::with_capacity(text.len());
  let mut last_index = 0;

  for (idx, _) in text.match_indices(target) {
    let prev_char = text[..idx].chars().next_back();
    let next_char = text[idx + target.len()..].chars().next();
    if is_token_delimiter(prev_char) && is_token_delimiter(next_char) {
      highlighted.push_str(&text[last_index..idx]);
      highlighted.push_str(&format!("{}", target.bright_yellow().bold()));
      last_index = idx + target.len();
    }
  }

  if last_index == 0 {
    text.to_string()
  } else {
    highlighted.push_str(&text[last_index..]);
    highlighted
  }
}

fn path_parent(path: &[usize]) -> Option<Vec<usize>> {
  if path.is_empty() {
    None
  } else {
    Some(path[..path.len() - 1].to_vec())
  }
}

fn get_node_at_path(code: &Cirru, path: &[usize]) -> Option<Cirru> {
  if path.is_empty() {
    return Some(code.clone());
  }
  let mut current = code;
  for &idx in path {
    match current {
      Cirru::List(items) => current = items.get(idx)?,
      Cirru::Leaf(_) => return None,
    }
  }
  Some(current.clone())
}

fn count_nodes_limited(node: &Cirru, limit: usize) -> usize {
  fn walk(node: &Cirru, acc: &mut usize, limit: usize) {
    if *acc >= limit {
      return;
    }
    *acc += 1;
    if let Cirru::List(items) = node {
      for item in items {
        if *acc >= limit {
          break;
        }
        walk(item, acc, limit);
      }
    }
  }
  let mut acc = 0;
  walk(node, &mut acc, limit);
  acc
}

fn can_show_parent_preview(expr_path: &[usize], parent_node: &Cirru) -> bool {
  if expr_path.len() > 8 {
    return false;
  }
  if let Cirru::List(items) = parent_node
    && items.len() > 8
  {
    return false;
  }
  count_nodes_limited(parent_node, 40) < 40
}

fn expression_and_parent_preview(
  code: &Cirru,
  match_path: &[usize],
  matched_node: &Cirru,
  highlight_target: Option<&str>,
  loose: bool,
) -> ((String, bool), Vec<(String, bool)>) {
  let expr_path = if matches!(matched_node, Cirru::Leaf(_)) {
    path_parent(match_path).unwrap_or_else(|| match_path.to_vec())
  } else {
    match_path.to_vec()
  };

  let expr_node = get_node_at_path(code, &expr_path).unwrap_or_else(|| matched_node.clone());
  let (expr_text, expr_truncated) = preview_node_oneline(&expr_node, 110);
  let expr_preview = (highlight_target_text(&expr_text, highlight_target, loose), expr_truncated);

  let mut parent_previews: Vec<(String, bool)> = Vec::new();
  let mut current_path = expr_path;

  for _ in 0..2 {
    let Some(parent_path) = path_parent(&current_path) else {
      break;
    };
    let Some(parent_node) = get_node_at_path(code, &parent_path) else {
      break;
    };

    if can_show_parent_preview(&parent_path, &parent_node) {
      let (preview_text, preview_truncated) = preview_node_oneline(&parent_node, 110);
      parent_previews.push((highlight_target_text(&preview_text, highlight_target, loose), preview_truncated));
    }

    current_path = parent_path;
  }

  (expr_preview, parent_previews)
}

/// Parse "namespace/definition" format into (namespace, definition)
/// Splits at the FIRST '/' so operator definitions like '/' and '/=' are handled correctly.
fn parse_target(target: &str) -> Result<(&str, &str), String> {
  target
    .split_once('/')
    .ok_or_else(|| format!("Invalid target format: '{target}'. Expected 'namespace/definition' (e.g. 'app.core/main')"))
}

fn resolve_search_cursor_references(
  input_path: &str,
  filter: &mut Option<String>,
  start_path: &mut Option<String>,
) -> Result<(), String> {
  if filter.as_deref() == Some("@cursor") || start_path.as_deref() == Some("@cursor") {
    let (cursor_target, cursor_path) = resolve_active_cursor_reference(input_path)?;
    if let Some(filter_target) = filter.as_deref()
      && filter_target != "@cursor"
      && filter_target != cursor_target
    {
      return Err(format!(
        "Cursor-scoped search targets '{cursor_target}', but --filter targets '{filter_target}'. Omit --filter or use --filter @cursor."
      ));
    }
    *filter = Some(cursor_target);
    if start_path.as_deref() == Some("@cursor") {
      *start_path = Some(cursor_path);
    }
  }
  Ok(())
}

fn resolve_query_cursor_references(cmd: &mut QueryCommand, input_path: &str) -> Result<(), String> {
  match &mut cmd.subcommand {
    QuerySubcommand::Def(opts) => opts.target = resolve_cursor_target_argument(input_path, &opts.target)?,
    QuerySubcommand::Peek(opts) => opts.target = resolve_cursor_target_argument(input_path, &opts.target)?,
    QuerySubcommand::Examples(opts) => opts.target = resolve_cursor_target_argument(input_path, &opts.target)?,
    QuerySubcommand::Tests(opts) => opts.target = resolve_cursor_target_argument(input_path, &opts.target)?,
    QuerySubcommand::Usages(opts) => opts.target = resolve_cursor_target_argument(input_path, &opts.target)?,
    QuerySubcommand::Search(opts) => {
      resolve_search_cursor_references(input_path, &mut opts.filter, &mut opts.start_path)?;
    }
    QuerySubcommand::SearchExpr(opts) => {
      resolve_search_cursor_references(input_path, &mut opts.filter, &mut opts.start_path)?;
    }
    QuerySubcommand::Schema(opts) => opts.target = resolve_cursor_target_argument(input_path, &opts.target)?,
    QuerySubcommand::Type(opts) => opts.target = resolve_cursor_target_argument(input_path, &opts.target)?,
    QuerySubcommand::TypeAt(opts) => {
      if opts.target == "@cursor" && opts.path == "@cursor" {
        (opts.target, opts.path) = resolve_active_cursor_reference(input_path)?;
      } else {
        opts.target = resolve_cursor_target_argument(input_path, &opts.target)?;
        opts.path = resolve_cursor_path_argument(input_path, &opts.target, &opts.path)?;
      }
    }
    QuerySubcommand::Context(opts) => opts.target = resolve_cursor_target_argument(input_path, &opts.target)?,
    QuerySubcommand::Ns(_)
    | QuerySubcommand::Defs(_)
    | QuerySubcommand::Pkg(_)
    | QuerySubcommand::Config(_)
    | QuerySubcommand::Error(_)
    | QuerySubcommand::Modules(_)
    | QuerySubcommand::Find(_)
    | QuerySubcommand::Next(_)
    | QuerySubcommand::Prev(_)
    | QuerySubcommand::HostProcs(_)
    | QuerySubcommand::Path(_)
    | QuerySubcommand::Anchors(_) => {}
  }
  Ok(())
}

pub fn handle_query_command(cmd: &QueryCommand, input_path: &str) -> Result<(), String> {
  let mut resolved = cmd.clone();
  resolve_query_cursor_references(&mut resolved, input_path)?;
  match &resolved.subcommand {
    QuerySubcommand::Ns(opts) => handle_ns(input_path, opts.namespace.as_deref(), opts.deps),
    QuerySubcommand::Defs(opts) => handle_defs(input_path, opts),
    QuerySubcommand::Pkg(_) => handle_pkg(input_path),
    QuerySubcommand::Config(_) => handle_config(input_path),
    QuerySubcommand::Error(_) => handle_error(input_path),
    QuerySubcommand::Modules(_) => handle_modules(input_path),
    QuerySubcommand::Def(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_def(input_path, ns, def, opts)
    }
    QuerySubcommand::Peek(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_peek(input_path, ns, def)
    }
    QuerySubcommand::Examples(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_examples(input_path, ns, def)
    }
    QuerySubcommand::Tests(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_tests(input_path, ns, def)
    }
    QuerySubcommand::Find(opts) => {
      if opts.exact {
        handle_find(input_path, &opts.symbol, opts.deps, opts.detail_offset)
      } else {
        handle_fuzzy_search(input_path, &opts.symbol, opts.deps, opts.limit, opts.detail_offset)
      }
    }
    QuerySubcommand::Usages(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_usages(input_path, ns, def, opts.deps, opts.detail_offset)
    }
    QuerySubcommand::Search(opts) => {
      let common_opts = SearchCommonOpts {
        filter: opts.filter.as_deref(),
        loose: !opts.exact,
        regex: opts.regex,
        max_depth: opts.max_depth,
        entry: opts.entry.as_deref(),
        detail_offset: opts.detail_offset,
        parent_path: opts.parent_path,
        format: parse_query_render_format(&opts.format)?,
        set_cursor: opts.set_cursor,
        compact_output: false,
      };
      handle_search_leaf(input_path, &opts.pattern, opts.start_path.as_deref(), &common_opts)
    }
    QuerySubcommand::SearchExpr(opts) => {
      let common_opts = SearchCommonOpts {
        filter: opts.filter.as_deref(),
        loose: !opts.exact,
        regex: false,
        max_depth: opts.max_depth,
        entry: opts.entry.as_deref(),
        detail_offset: opts.detail_offset,
        parent_path: false,
        format: parse_query_render_format(&opts.format)?,
        set_cursor: opts.set_cursor,
        compact_output: false,
      };
      handle_search_expr(input_path, &opts.pattern, opts.json, opts.start_path.as_deref(), &common_opts)
    }
    QuerySubcommand::Next(_) => handle_repeat_cursor_search(input_path, true),
    QuerySubcommand::Prev(_) => handle_repeat_cursor_search(input_path, false),
    QuerySubcommand::Schema(opts) => {
      let (ns, def) = parse_target(&opts.target)?;
      handle_schema(input_path, ns, def, opts.json)
    }
    QuerySubcommand::Type(opts) => handle_type(input_path, opts),
    QuerySubcommand::TypeAt(opts) => handle_type_at(input_path, opts),
    QuerySubcommand::Context(opts) => handle_context(input_path, opts),
    QuerySubcommand::HostProcs(opts) => handle_host_procs(opts),
    QuerySubcommand::Path(opts) => handle_query_path(input_path, opts),
    QuerySubcommand::Anchors(opts) => handle_query_anchors(input_path, opts),
  }
}

fn prepare_program_for_type_query(snapshot: &snapshot::Snapshot) -> Result<(), String> {
  program::clear_runtime_caches_for_reload(Arc::from("query.type"), Arc::from("query.type"), true)?;
  {
    let mut program_data = program::PROGRAM_CODE_DATA
      .write()
      .map_err(|_| "Failed to open program data".to_owned())?;
    *program_data = program::extract_program_data(snapshot)?;
  }

  let warnings = RefCell::<Vec<LocatedWarning>>::new(vec![]);
  runner::preprocess::ensure_ns_def_compiled(
    calcit::calcit::CORE_NS,
    calcit::calcit::BUILTIN_IMPLS_ENTRY,
    &warnings,
    &CallStackList::default(),
  )
  .map_err(|error| error.msg)
}

fn parse_type_annotation_query(target: &str) -> Result<Arc<CalcitTypeAnnotation>, String> {
  const LEGACY_SIMPLE_TYPES: &[&str] = &[
    "any",
    "bool",
    "number",
    "string",
    "symbol",
    "tag",
    "list",
    "map",
    "set",
    "tuple",
    "fn",
    "ref",
    "buffer",
    "cirru-quote",
    "unit",
    "nil",
    "js-object",
    "dynamic",
  ];

  let trimmed = target.trim();
  if trimmed.is_empty() {
    return Err("Type query cannot be empty".to_owned());
  }
  if trimmed.starts_with('(') {
    return Err(format!(
      "Type annotation `{target}` has an extra outer parenthesis layer. Use Cirru directly, for example `:: 'List 'Number`."
    ));
  }
  if !trimmed.chars().any(char::is_whitespace) {
    let is_quoted_symbol = trimmed.starts_with('\'');
    let name = trimmed.strip_prefix(':').or_else(|| trimmed.strip_prefix('\'')).unwrap_or(trimmed);
    let is_legacy_type_name = LEGACY_SIMPLE_TYPES.contains(&name);
    let is_builtin_type_symbol = is_quoted_symbol
      && !name.starts_with(':')
      && !name.starts_with('\'')
      && CalcitTypeAnnotation::canonical_type_symbol_name(name).is_some();
    if !is_legacy_type_name && !is_builtin_type_symbol {
      return Err(format!(
        "Unknown builtin type `{target}`. Use a builtin type symbol such as `'Number`, a type expression such as `:: 'List 'Number`, or `namespace/definition`."
      ));
    }
    return Ok(Arc::new(CalcitTypeAnnotation::from_tag_name(name)));
  }

  let nodes = cirru_parser::parse(trimmed).map_err(|error| format!("Failed to parse type annotation `{target}`: {error}"))?;
  if nodes.len() != 1 {
    return Err(format!("Type annotation must contain exactly one expression, got {}", nodes.len()));
  }
  let form = code_to_calcit(&nodes[0], "query.type", "target", vec![])?;
  Ok(CalcitTypeAnnotation::parse_type_annotation_form(&form))
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod type_query_tests {
  use super::*;
  use crate::cli_handlers::test_support::TestProject;

  fn on_cli_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
      .name("calcit-query-test".into())
      // Core preprocessing and context rendering are recursive; mirror the
      // CLI worker instead of relying on Rust's smaller test-thread stack.
      .stack_size(16 * 1024 * 1024)
      .spawn(f)
      .expect("query test thread should start")
      .join()
      .expect("query test thread should finish")
  }

  fn prepare_program_for_type_query_on_cli_stack(snapshot: snapshot::Snapshot) {
    on_cli_stack(move || prepare_program_for_type_query(&snapshot)).expect("static type metadata should prepare");
  }

  #[test]
  fn parses_simple_type_tag_without_treating_it_as_a_call() {
    let annotation = parse_type_annotation_query(":number").expect("number type should parse");
    assert!(matches!(annotation.as_ref(), CalcitTypeAnnotation::Number));
  }

  #[test]
  fn parses_canonical_builtin_type_symbol_without_treating_it_as_a_type_variable() {
    let annotation = parse_type_annotation_query("'String").expect("String type symbol should parse");
    assert!(matches!(annotation.as_ref(), CalcitTypeAnnotation::String));
  }

  #[test]
  fn unknown_builtin_type_suggests_canonical_symbols() {
    let error = parse_type_annotation_query("String").expect_err("unquoted String should be rejected");
    assert_eq!(
      error,
      "Unknown builtin type `String`. Use a builtin type symbol such as `'Number`, a type expression such as `:: 'List 'Number`, or `namespace/definition`."
    );
  }

  #[test]
  fn rejects_excess_type_symbol_prefixes() {
    assert!(parse_type_annotation_query("''String").is_err());
    assert!(parse_type_annotation_query("':String").is_err());
  }

  #[test]
  fn parses_any_as_the_legacy_dynamic_alias() {
    let annotation = parse_type_annotation_query(":any").expect("any type should parse");
    assert_eq!(annotation.to_brief_string(), "dynamic");
    assert_eq!(
      format_type_query_annotation(annotation.as_ref()).expect("alias should format"),
      "'Dynamic"
    );
    assert!(matches!(annotation.as_ref(), CalcitTypeAnnotation::Dynamic));
    assert!(CalcitTypeAnnotation::String.matches_annotation(annotation.as_ref()));
    assert!(annotation.matches_annotation(&CalcitTypeAnnotation::String));
  }

  #[test]
  fn parses_compound_type_with_cirru_structure() {
    let annotation = parse_type_annotation_query(":: :list :number").expect("compound list type should parse");
    assert!(matches!(annotation.as_ref(), CalcitTypeAnnotation::List(inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::Number)));

    let annotation = parse_type_annotation_query(":: 'List 'Number").expect("canonical compound list type should parse");
    assert!(matches!(annotation.as_ref(), CalcitTypeAnnotation::List(inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::Number)));
  }

  #[test]
  fn rejects_extra_outer_parenthesis_layer() {
    let error = parse_type_annotation_query("(:: :list :number)").expect_err("extra call layer should be rejected");
    assert!(error.contains("extra outer parenthesis"));
  }

  #[test]
  fn renders_type_annotations_without_edn_top_level_wrapper() {
    assert_eq!(
      format_type_query_annotation(&CalcitTypeAnnotation::Number).expect("number should format"),
      "'Number"
    );
    let list_type = CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Number));
    assert_eq!(
      format_type_query_annotation(&list_type).expect("list should format"),
      ":: 'List 'Number"
    );
    assert_eq!(format_query_schema(&CalcitTypeAnnotation::Ref(DYNAMIC_TYPE.clone()), true), "'Ref");
  }

  #[test]
  fn schema_json_is_a_versioned_machine_readable_envelope() {
    let annotation = CalcitTypeAnnotation::Ref(Arc::new(CalcitTypeAnnotation::Bool));
    let output =
      format_schema_query_json("app.main/*enabled?", "local", &annotation, "revision-1".to_owned()).expect("schema JSON should format");
    let value: serde_json::Value = serde_json::from_str(&output).expect("schema output should be valid JSON");

    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["command"], "query.schema");
    assert_eq!(value["data"]["id"], "app.main/*enabled?");
    assert_eq!(value["data"]["canonical_schema"], ":: 'Ref 'Bool");
    assert_eq!(value["data"]["tree"], serde_json::json!(["::", "'Ref", "'Bool"]));

    let broad_ref = CalcitTypeAnnotation::Ref(DYNAMIC_TYPE.clone());
    let output =
      format_schema_query_json("app.main/*cache", "local", &broad_ref, "revision-2".to_owned()).expect("broad ref JSON should format");
    let value: serde_json::Value = serde_json::from_str(&output).expect("schema output should be valid JSON");
    assert_eq!(value["data"]["canonical_schema"], "'Ref");
    assert_eq!(value["data"]["tree"], "'Ref");
  }

  #[test]
  fn examples_render_leaf_nodes_without_formatter_failure() {
    assert_eq!(format_example_node(&Cirru::Leaf("|literal".into())), "|literal");
    assert_eq!(
      format_example_node(&Cirru::List(vec![Cirru::Leaf("inc".into()), Cirru::Leaf("1".into())])),
      "inc 1"
    );
  }

  #[test]
  fn search_results_share_cursor_indices_and_can_select_one() {
    let fixture = TestProject::from_fixture();
    let snapshot_path = fixture.snapshot_string();

    let results = vec![(
      "app.main".to_string(),
      "main!".to_string(),
      vec![(vec![48, 0], Cirru::leaf("do")), (vec![48, 1], Cirru::leaf("true"))],
    )];
    let saved_query = CursorLastQuery {
      command: "search".to_string(),
      pattern: "true".to_string(),
      filter: Some("app.main/main!".to_string()),
      exact: true,
      regex: false,
      max_depth: 0,
      start_path: None,
      entry: None,
      pattern_is_json: false,
      selected_index: 1,
      snapshot_revision: snapshot_content_revision(&snapshot_path).expect("snapshot revision should compute"),
    };
    maybe_set_cursor_from_search_results(&snapshot_path, &results, 1, saved_query.clone())
      .expect("second search result should become cursor");
    assert_eq!(
      crate::cli_handlers::cursor::resolve_cursor_path_argument(&snapshot_path, "app.main/main!", "@cursor")
        .expect("query-selected cursor should resolve"),
      "@48.1"
    );

    let mut filter = None;
    let mut start_path = Some("@cursor".to_string());
    resolve_search_cursor_references(&snapshot_path, &mut filter, &mut start_path)
      .expect("cursor-scoped search should infer target and path");
    assert_eq!(filter.as_deref(), Some("app.main/main!"));
    assert_eq!(start_path.as_deref(), Some("@48.1"));

    let mut type_at = QueryCommand {
      subcommand: QuerySubcommand::TypeAt(QueryTypeAtCommand {
        target: "@cursor".to_string(),
        path: "@cursor".to_string(),
        format: "json".to_string(),
      }),
    };
    resolve_query_cursor_references(&mut type_at, &snapshot_path).expect("type-at should resolve cursor target and path");
    let QuerySubcommand::TypeAt(type_at) = type_at.subcommand else {
      panic!("type-at command should remain type-at")
    };
    assert_eq!(type_at.target, "app.main/main!");
    assert_eq!(type_at.path, "@48.1");

    let mut mismatched_filter = Some("app.other/demo".to_string());
    let mut cursor_start = Some("@cursor".to_string());
    let error = resolve_search_cursor_references(&snapshot_path, &mut mismatched_filter, &mut cursor_start)
      .expect_err("cursor search should reject mismatched target");
    assert!(error.contains("Cursor-scoped search targets"), "error: {error}");

    let error = maybe_set_cursor_from_search_results(&snapshot_path, &results, 2, saved_query)
      .expect_err("out-of-range search cursor index should fail");
    assert!(error.contains("returned 2 match"), "error: {error}");

    let snapshot = load_main_snapshot(&snapshot_path).expect("query cursor fixture should load");
    let options = SearchCommonOpts {
      filter: Some("app.main/main!"),
      loose: false,
      regex: false,
      max_depth: 0,
      entry: None,
      detail_offset: 0,
      parent_path: false,
      format: QueryRenderFormat::Json,
      set_cursor: None,
      compact_output: false,
    };
    let output = format_search_results_json("query.search", "true", false, None, &options, &snapshot, &results)
      .expect("search JSON should format");
    let value: serde_json::Value = serde_json::from_str(&output).expect("search output should be valid JSON");
    assert_eq!(value["data"]["definitions"][0]["matches"][0]["cursor_index"], 0);
    assert_eq!(value["data"]["definitions"][0]["matches"][1]["cursor_index"], 1);

    handle_repeat_cursor_search(&snapshot_path, false).expect("saved search should recompute its previous result");
    assert_eq!(
      load_cursor_last_query(&snapshot_path)
        .expect("repeated query should remain saved")
        .selected_index,
      0
    );
    let mut changed_snapshot = std::fs::read_to_string(&fixture.path).expect("snapshot should read for revision change");
    changed_snapshot.push('\n');
    std::fs::write(&fixture.path, changed_snapshot).expect("snapshot revision should change");
    let error = handle_repeat_cursor_search(&snapshot_path, true).expect_err("changed snapshot should reject a stale result index");
    assert!(error.contains("Snapshot changed since the saved cursor search"), "error: {error}");
  }

  #[test]
  fn number_type_query_uses_static_dispatch_metadata() {
    let _guard = crate::GLOBAL_TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let snapshot = load_core_snapshot().expect("core snapshot should load");
    prepare_program_for_type_query_on_cli_stack(snapshot);

    let methods = runner::preprocess::static_method_descriptors(&CalcitTypeAnnotation::Number)
      .expect("number method metadata should resolve")
      .into_iter()
      .map(|method| method.name)
      .collect::<Vec<_>>();

    assert_eq!(
      methods,
      vec![
        ".ceil",
        ".compare",
        ".display-by",
        ".empty",
        ".floor",
        ".format",
        ".fract",
        ".inc",
        ".negate",
        ".pow",
        ".rem",
        ".round",
        ".round?",
        ".sqrt",
        ".debug",
        ".eq?",
        ".add",
        ".multiply",
      ]
    );
  }

  #[test]
  fn query_format_and_unicode_truncation_are_deterministic() {
    assert_eq!(parse_query_render_format("human"), Ok(QueryRenderFormat::Human));
    assert_eq!(parse_query_render_format("json"), Ok(QueryRenderFormat::Json));
    assert!(parse_query_render_format("edn").is_err());
    assert_eq!(truncate_chars("你好 Calcit", 2), ("你好…".to_owned(), true));
    assert_eq!(truncate_chars("你好", 2), ("你好".to_owned(), false));
  }

  fn context_test_options(target: &str) -> QueryContextCommand {
    QueryContextCommand {
      target: target.to_owned(),
      budget: 2400,
      format: "json".to_owned(),
      deps: false,
      dependency_limit: 12,
      usage_limit: 8,
      example_limit: 3,
      test_limit: 3,
    }
  }

  #[test]
  fn special_builtin_context_preserves_examples_and_intent() {
    let _guard = crate::GLOBAL_TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let snapshot = load_core_snapshot().expect("core snapshot should load");
    let envelope = on_cli_stack(move || {
      let meta = lookup_special_builtin_query_meta("calcit.core", "to-js-data")
        .expect("metadata lookup should work")
        .expect("to-js-data metadata should exist");
      build_special_builtin_context(
        &snapshot,
        "calcit.core",
        "to-js-data",
        meta,
        &context_test_options("calcit.core/to-js-data"),
      )
      .expect("context should build")
    });

    assert_eq!(envelope.command, "query.context");
    assert_eq!(envelope.data.coverage, "intentional-dynamic");
    assert_eq!(envelope.data.examples.total, 3);
    assert_eq!(
      envelope.data.examples.items[2]
        .tree
        .as_ref()
        .and_then(|tree| tree.as_array())
        .map(Vec::len),
      Some(3)
    );
    assert!(
      envelope
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "I_DYNAMIC_TYPE_INTENTIONAL")
    );
  }

  #[test]
  fn regular_context_carries_snapshot_revision_and_tree_location() {
    let _guard = crate::GLOBAL_TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let snapshot = load_core_snapshot().expect("core snapshot should load");
    let (envelope, expected_revision) = on_cli_stack(move || {
      let entry = snapshot
        .files
        .get("calcit.core")
        .and_then(|file| file.defs.get("map"))
        .expect("core map should exist");
      let expected_revision = snapshot::definition_revision(entry).expect("revision should compute");
      let envelope = build_regular_context(&snapshot, "calcit.core", "map", entry, &context_test_options("calcit.core/map"))
        .expect("context should build");
      (envelope, expected_revision)
    });

    assert_eq!(envelope.data.id, "calcit.core/map");
    assert_eq!(envelope.data.source, "core");
    assert_eq!(envelope.data.code.root, "code");
    assert!(envelope.data.code.nodes > 0);
    assert_eq!(envelope.revision, expected_revision);
  }

  #[test]
  fn project_function_schema_argument_resolves_source_backed_struct() {
    let _guard = crate::GLOBAL_TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let snapshot = load_snapshot("calcit/test.cirru").expect("test snapshot should load");
    prepare_program_for_type_query_on_cli_stack(snapshot.clone());
    let schema = program::lookup_def_schema("test-struct.main", "sum-point");
    let CalcitTypeAnnotation::Fn(annotation) = schema.as_ref() else {
      panic!("sum-point should have a function schema");
    };
    let argument_type = annotation.arg_types.first().expect("sum-point argument type");
    let struct_def = argument_type.resolve_to_struct().expect("source-backed Point2D should resolve");

    assert_eq!(struct_def.name.ref_str(), "Point2D");
    assert_eq!(struct_def.index_of("x"), Some(0));
    assert!(matches!(
      struct_def.field_types.first().map(AsRef::as_ref),
      Some(CalcitTypeAnnotation::Number)
    ));
    let result_type = CalcitTypeAnnotation::TypeRef(Arc::from("test-enum.main/Result0"), Arc::new(vec![]));
    assert!(result_type.resolve_to_enum().is_some(), "source-backed Result0 should resolve");
    assert!(
      result_type.matches_annotation(&CalcitTypeAnnotation::AnonymousEnum),
      "a source-backed enum reference should satisfy enum operations"
    );
    let person_symbol =
      code_to_calcit(&Cirru::Leaf(Arc::from("Person")), "test-struct.main", "test-struct", vec![]).expect("Person symbol should parse");
    let person_type = runner::preprocess::infer_static_type_from_expr(&person_symbol).expect("Person type should infer statically");
    assert!(
      matches!(person_type.as_ref(), CalcitTypeAnnotation::TypeRef(name, _) if name.as_ref() == "test-struct.main/Person"),
      "data definitions should remain named type refs rather than synthetic struct instances: {person_type}"
    );
    let (queried_person_type, source) =
      resolve_type_query_target(&snapshot, "test-struct.main/Person").expect("type query should infer defstruct type");
    assert_eq!(source, "definition inference");
    assert!(
      matches!(queried_person_type.as_ref(), CalcitTypeAnnotation::TypeRef(name, _) if name.as_ref() == "test-struct.main/Person"),
      "query type should expose the source-backed struct type: {queried_person_type}"
    );

    let warnings = RefCell::new(vec![]);
    runner::preprocess::compile_source_def_for_snapshot("test-struct.main", "sum-point", &warnings, &CallStackList::default())
      .expect("sum-point should preprocess");
    let compiled = program::lookup_compiled_def("test-struct.main", "sum-point").expect("compiled sum-point");
    let target = find_preprocessed_node_at_path(&compiled.preprocessed_code, "test-struct.main", "sum-point", &[3, 1], true)
      .expect("field access expression should retain a source path");
    let Calcit::List(field_access) = target else {
      panic!("field access should remain a call");
    };
    assert!(
      matches!(field_access.first(), Some(Calcit::Proc(calcit::CalcitProc::NativeStructNth))),
      "typed tag access should specialize to struct nth, got {target}"
    );
  }

  #[test]
  fn type_at_path_parser_accepts_semantic_code_prefixes() {
    assert_eq!(parse_type_at_path("code@3.2"), Ok(vec![3, 2]));
    assert_eq!(parse_type_at_path("@3.2"), Ok(vec![3, 2]));
    assert_eq!(parse_type_at_path("3.2"), Ok(vec![3, 2]));
  }

  #[test]
  fn definition_return_schema_does_not_leak_into_nested_functions() {
    let mut nodes = cirru_parser::parse("fn ()\n  fn () 1").expect("test function should parse");
    let mut entry = snapshot::CodeEntry::from_code(nodes.remove(0));
    entry.schema = Arc::new(CalcitTypeAnnotation::from_function_parts(
      vec![],
      Arc::new(CalcitTypeAnnotation::String),
    ));

    let root_expected = expected_type_at_path(&entry, None, "tests.type-at", "demo", &[2])
      .expect("root body should inherit the definition return schema");
    assert!(matches!(root_expected.0.as_ref(), CalcitTypeAnnotation::String));
    assert!(
      expected_type_at_path(&entry, None, "tests.type-at", "demo", &[2, 2]).is_none(),
      "nested fn body must use its own context rather than the enclosing definition schema"
    );
  }

  #[test]
  fn type_at_reports_expected_type_mismatches_without_rejecting_optional_values() {
    let mismatch = type_at_expected_mismatch_diagnostic(
      &CalcitTypeAnnotation::Number,
      &CalcitTypeAnnotation::String,
      "callable parameter",
      "code@3.1",
    )
    .expect("number should not satisfy string");
    assert_eq!(mismatch.code, "W_TYPE_AT_EXPECTED_MISMATCH");
    assert_eq!(mismatch.path.as_deref(), Some("code@3.1"));

    assert!(
      type_at_expected_mismatch_diagnostic(
        &CalcitTypeAnnotation::Number,
        &CalcitTypeAnnotation::Optional(Arc::new(CalcitTypeAnnotation::Number)),
        "return schema",
        "code@3",
      )
      .is_none(),
      "a concrete number satisfies optional<number>"
    );
  }
}

fn definition_type_query_target(target: &str) -> Option<(&str, &str)> {
  let (namespace, definition) = target.split_once('/')?;
  if namespace.is_empty() || definition.is_empty() || target.chars().any(char::is_whitespace) {
    None
  } else {
    Some((namespace, definition))
  }
}

fn resolve_type_query_target(snapshot: &snapshot::Snapshot, target: &str) -> Result<(Arc<CalcitTypeAnnotation>, &'static str), String> {
  if let Some((namespace, definition)) = definition_type_query_target(target) {
    let file = snapshot
      .files
      .get(namespace)
      .ok_or_else(|| format!("Namespace `{namespace}` not found"))?;
    if !file.defs.contains_key(definition) {
      return Err(format!("Definition `{target}` not found"));
    }

    let annotation = program::lookup_def_schema(namespace, definition);
    let is_data_definition_marker = matches!(
      annotation.as_ref(),
      CalcitTypeAnnotation::Custom(value)
        if matches!(value.as_ref(), Calcit::Tag(tag) if matches!(tag.ref_str(), "struct-def" | "enum-def"))
    );
    if !matches!(annotation.as_ref(), CalcitTypeAnnotation::Dynamic) && !is_data_definition_marker {
      return Ok((annotation, "definition schema"));
    }

    let symbol = code_to_calcit(&Cirru::Leaf(Arc::from(definition)), namespace, "&query:type", vec![])?;
    if let Some(inferred) = runner::preprocess::infer_static_type_from_expr(&symbol)
      && !matches!(inferred.as_ref(), CalcitTypeAnnotation::Dynamic)
    {
      return Ok((inferred, "definition inference"));
    }

    return Err(format!(
      "Definition `{target}` has neither an explicit schema nor an inferable static type. Add a schema or query a concrete type annotation such as `'Number`."
    ));
  }

  Ok((parse_type_annotation_query(target)?, "type annotation"))
}

fn format_type_query_annotation(annotation: &CalcitTypeAnnotation) -> Result<String, String> {
  cirru_edn::format(&annotation.to_type_edn(), true)
    .map(|text| {
      let rendered = text.trim();
      rendered.strip_prefix("do ").unwrap_or(rendered).to_owned()
    })
    .map_err(|error| format!("Failed to format resolved type: {error}"))
}

fn handle_type(input_path: &str, opts: &QueryTypeCommand) -> Result<(), String> {
  let format = parse_query_render_format(&opts.format)?;
  let snapshot = match definition_type_query_target(&opts.target) {
    Some((namespace, _)) if namespace == calcit::calcit::CORE_NS || namespace == "calcit.internal" => load_core_snapshot()?,
    Some(_) => load_snapshot(input_path)?,
    None => load_core_snapshot()?,
  };
  prepare_program_for_type_query(&snapshot)?;
  let (annotation, source) = resolve_type_query_target(&snapshot, &opts.target)?;
  let rendered_type = format_type_query_annotation(annotation.as_ref())?;
  let methods = runner::preprocess::static_method_descriptors(annotation.as_ref()).map(|items| {
    items
      .into_iter()
      .map(|method| ContextMethod {
        name: method.name,
        origin: method.origin,
      })
      .collect::<Vec<_>>()
  });
  let method_fingerprint = methods
    .as_ref()
    .map(|items| {
      items
        .iter()
        .map(|method| format!("{}@{}", method.name, method.origin))
        .collect::<Vec<_>>()
        .join("\n")
    })
    .unwrap_or_else(|| "unknown".to_owned());
  let revision = definition_type_query_target(&opts.target)
    .and_then(|(namespace, definition)| snapshot.files.get(namespace).and_then(|file| file.defs.get(definition)))
    .map(snapshot::definition_revision)
    .transpose()?
    .unwrap_or_else(|| semantic_revision(&[&rendered_type, &method_fingerprint]));

  let data = TypeQueryData {
    target: opts.target.clone(),
    canonical_type: rendered_type,
    resolved_from: source,
    methods,
  };
  let uses_legacy_any = definition_type_query_target(&opts.target).is_none()
    && opts
      .target
      .split(|ch: char| ch.is_whitespace() || matches!(ch, '(' | ')' | '[' | ']' | '{' | '}'))
      .any(|token| matches!(token, "any" | ":any"));
  let diagnostics = if uses_legacy_any {
    vec![ContextDiagnostic {
      code: "W_LEGACY_ANY_ALIAS".to_owned(),
      phase: "analysis",
      severity: "warning",
      message: "`:any` is a legacy alias for `:dynamic`; the canonical type and generated schema use `'Dynamic`. Do not use it to model polymorphism.".to_owned(),
      path: None,
      intent: Some("migration".to_owned()),
    }]
  } else {
    vec![]
  };

  if format == QueryRenderFormat::Json {
    let envelope = SemanticQueryEnvelope {
      schema_version: 1,
      command: "query.type",
      revision,
      data,
      diagnostics,
      next: vec![],
    };
    println!(
      "{}",
      serde_json::to_string_pretty(&envelope).map_err(|error| format!("Failed to encode type query result: {error}"))?
    );
    return Ok(());
  }

  if data.canonical_type.contains('\n') {
    println!("{}\n{}", "Type:".bold(), data.canonical_type);
  } else {
    println!("{} {}", "Type:".bold(), data.canonical_type);
  }
  println!("{} {}", "Resolved from:".bold(), data.resolved_from);
  println!("{} {revision}", "Revision:".bold());
  if uses_legacy_any {
    println!(
      "{}",
      "Warning [W_LEGACY_ANY_ALIAS]: `:any` is the legacy spelling of `:dynamic`; use `'Dynamic` only for genuinely dynamic boundaries, or use `:generics`/TypeVar or trait `:where` for polymorphism."
        .yellow()
    );
  }

  match data.methods {
    Some(methods) if methods.is_empty() => {
      println!("{} 0", "Methods:".bold());
      println!("  (no methods registered for this type)");
    }
    Some(methods) => {
      println!("{} {} (high → low precedence)", "Methods:".bold(), methods.len());
      for method in methods {
        println!("  {:<20} {}", method.name, format!("({})", method.origin).dimmed());
      }
    }
    None => {
      println!("{} unknown", "Methods:".bold());
      println!("  This type has no statically resolvable method metadata.");
    }
  }

  Ok(())
}

fn parse_type_at_path(raw: &str) -> Result<Vec<usize>, String> {
  let path = raw.strip_prefix("code").unwrap_or(raw);
  parse_path(path)
}

fn semantic_code_path(path: &[usize]) -> String {
  if path.is_empty() {
    "code".to_owned()
  } else {
    format!("code@{}", path.iter().map(usize::to_string).collect::<Vec<_>>().join("."))
  }
}

fn format_cirru_expression(node: &Cirru) -> String {
  cirru_parser::format(std::slice::from_ref(node), true.into())
    .map(|text| text.trim().to_owned())
    .unwrap_or_else(|_| format!("{node:?}"))
}

fn calcit_direct_source_path(node: &Calcit, namespace: &str, definition: &str) -> Option<Vec<usize>> {
  let location = node.get_location()?;
  if location.ns.as_ref() != namespace || location.def.as_ref() != definition {
    return None;
  }
  Some(location.coord.iter().map(|idx| *idx as usize).collect())
}

/// Recover the source path represented by a preprocessed expression. Imports, procs and literals
/// do not carry locations, so list paths are derived from the nearest direct child that does.
fn calcit_expression_source_path(node: &Calcit, namespace: &str, definition: &str) -> Option<Vec<usize>> {
  if let Some(path) = calcit_direct_source_path(node, namespace, definition) {
    return Some(path);
  }
  let Calcit::List(items) = node else {
    return None;
  };
  for child in items.iter() {
    if let Some(mut child_path) = calcit_expression_source_path(child, namespace, definition) {
      child_path.pop();
      return Some(child_path);
    }
  }
  None
}

fn find_preprocessed_node_at_path<'a>(
  node: &'a Calcit,
  namespace: &str,
  definition: &str,
  target_path: &[usize],
  target_is_list: bool,
) -> Option<&'a Calcit> {
  let shape_matches = matches!(node, Calcit::List(_)) == target_is_list;
  if shape_matches
    && calcit_expression_source_path(node, namespace, definition)
      .as_deref()
      .is_some_and(|path| path == target_path)
  {
    return Some(node);
  }

  if let Calcit::List(items) = node {
    for child in items.iter() {
      if let Some(found) = find_preprocessed_node_at_path(child, namespace, definition, target_path, target_is_list) {
        return Some(found);
      }
    }
  }
  None
}

#[derive(Debug, Clone)]
struct StaticCallableSignature {
  arg_types: Vec<Arc<CalcitTypeAnnotation>>,
  rest_type: Option<Arc<CalcitTypeAnnotation>>,
}

impl StaticCallableSignature {
  fn from_fn_annotation(annotation: &CalcitFnTypeAnnotation) -> Self {
    Self {
      arg_types: annotation.arg_types.clone(),
      rest_type: annotation.rest_type.clone(),
    }
  }

  fn expected_arg(&self, index: usize) -> Option<Arc<CalcitTypeAnnotation>> {
    if let Some(value) = self.arg_types.get(index) {
      return match value.as_ref() {
        CalcitTypeAnnotation::Variadic(inner) => Some(inner.clone()),
        _ => Some(value.clone()),
      };
    }
    self.rest_type.clone().or_else(|| {
      self.arg_types.last().and_then(|value| match value.as_ref() {
        CalcitTypeAnnotation::Variadic(inner) => Some(inner.clone()),
        _ => None,
      })
    })
  }
}

fn callable_signature_from_head(head: &Calcit) -> Option<StaticCallableSignature> {
  match head {
    Calcit::Proc(proc) => {
      let signature = proc.get_type_signature()?;
      Some(StaticCallableSignature {
        arg_types: signature.arg_types.clone(),
        rest_type: None,
      })
    }
    Calcit::Fn { info, .. } => Some(StaticCallableSignature {
      arg_types: info.arg_types.clone(),
      rest_type: info.rest_type.clone(),
    }),
    Calcit::Local(local) => local
      .type_info
      .resolve_to_fn()
      .map(|annotation| StaticCallableSignature::from_fn_annotation(annotation.as_ref())),
    Calcit::Import(import) => program::lookup_def_schema(&import.ns, &import.def)
      .resolve_to_fn()
      .map(|annotation| StaticCallableSignature::from_fn_annotation(annotation.as_ref())),
    Calcit::Symbol { sym, info, .. } => program::lookup_def_schema(&info.at_ns, sym)
      .resolve_to_fn()
      .map(|annotation| StaticCallableSignature::from_fn_annotation(annotation.as_ref())),
    _ => None,
  }
}

fn expected_type_at_path(
  entry: &snapshot::CodeEntry,
  processed_root: Option<&Calcit>,
  namespace: &str,
  definition: &str,
  target_path: &[usize],
) -> Option<(Arc<CalcitTypeAnnotation>, String)> {
  let (&target_index, parent_path) = target_path.split_last()?;
  let parent_node = navigate_to_path(&entry.code, parent_path).ok()?;
  let Cirru::List(parent_items) = parent_node else {
    return None;
  };
  let head_name = parent_items.first().and_then(|head| match head {
    Cirru::Leaf(name) => Some(name.as_ref()),
    Cirru::List(_) => None,
  });

  if parent_path.is_empty()
    && matches!(head_name, Some("defn" | "fn"))
    && target_index == parent_items.len().saturating_sub(1)
    && let CalcitTypeAnnotation::Fn(annotation) = entry.schema.as_ref()
  {
    return Some((annotation.return_type.clone(), "definition return schema".to_owned()));
  }
  if matches!(head_name, Some("if" | "&if")) && target_index == 1 {
    return Some((Arc::new(CalcitTypeAnnotation::Bool), "if condition".to_owned()));
  }

  let processed_parent = processed_root.and_then(|root| {
    find_preprocessed_node_at_path(root, namespace, definition, parent_path, true).and_then(|node| match node {
      Calcit::List(items) => Some(items),
      _ => None,
    })
  })?;

  if matches!(processed_parent.first(), Some(Calcit::Syntax(calcit::CalcitSyntax::AssertType, _)))
    && target_index == 1
    && let Some(type_form) = processed_parent.get(2)
  {
    return Some((
      CalcitTypeAnnotation::parse_type_annotation_form(type_form),
      "assert-type annotation".to_owned(),
    ));
  }

  let signature = callable_signature_from_head(processed_parent.first()?)?;
  let arg_index = target_index.checked_sub(1)?;
  signature
    .expected_arg(arg_index)
    .map(|expected| (expected, "callable parameter".to_owned()))
}

fn type_at_evidence(node: &Calcit, path: &str, used_preprocessed: bool) -> TypeAtEvidence {
  let kind = match node {
    Calcit::Number(_) | Calcit::Str(_) | Calcit::Bool(_) | Calcit::Nil | Calcit::Tag(_) => "literal",
    Calcit::Local(_) => "local-binding",
    Calcit::Import(_) | Calcit::Symbol { .. } => "definition-schema",
    Calcit::Proc(_) => "proc-signature",
    Calcit::Fn { .. } => "function-schema",
    Calcit::List(items) => match items.first() {
      Some(Calcit::Proc(calcit::CalcitProc::List | calcit::CalcitProc::Set | calcit::CalcitProc::NativeMap)) => "collection-literal",
      Some(Calcit::Proc(_)) => "proc-return",
      Some(Calcit::Import(_) | Calcit::Symbol { .. }) => "definition-return",
      Some(Calcit::Local(_)) => "local-function-return",
      Some(Calcit::Method(_, _)) => "method-dispatch",
      Some(Calcit::Syntax(_, _)) => "syntax-rule",
      _ => "expression-synthesis",
    },
    _ => "value-shape",
  };
  TypeAtEvidence {
    kind: kind.to_owned(),
    detail: if used_preprocessed {
      "inferred from preprocessed code and lexical type metadata".to_owned()
    } else {
      "inferred from the selected source subtree; lexical bindings were unavailable".to_owned()
    },
    path: Some(path.to_owned()),
  }
}

fn collect_type_at_bindings(node: &Calcit, values: &mut BTreeMap<String, (Arc<CalcitTypeAnnotation>, Option<String>)>) {
  match node {
    Calcit::Local(local) => {
      let path = local
        .location
        .as_ref()
        .map(|coord| semantic_code_path(&coord.iter().map(|idx| *idx as usize).collect::<Vec<_>>()));
      let should_replace = values
        .get(local.sym.as_ref())
        .is_none_or(|(current, _)| matches!(current.as_ref(), CalcitTypeAnnotation::Dynamic));
      if should_replace {
        values.insert(local.sym.to_string(), (local.type_info.clone(), path));
      }
    }
    Calcit::List(items) => {
      for child in items.iter() {
        collect_type_at_bindings(child, values);
      }
    }
    _ => {}
  }
}

fn format_type_at_bindings(processed_root: Option<&Calcit>, target: Option<&Calcit>) -> Result<Vec<TypeAtBinding>, String> {
  let mut values: BTreeMap<String, (Arc<CalcitTypeAnnotation>, Option<String>)> = BTreeMap::new();
  if let Some(Calcit::List(root_items)) = processed_root
    && let Some(Calcit::List(params)) = root_items.get(2)
  {
    collect_type_at_bindings(&Calcit::List(params.clone()), &mut values);
  }
  if let Some(target) = target {
    collect_type_at_bindings(target, &mut values);
  }

  values
    .into_iter()
    .map(|(name, (annotation, path))| {
      Ok(TypeAtBinding {
        name,
        r#type: format_type_query_annotation(annotation.as_ref())?,
        path,
      })
    })
    .collect()
}

fn type_at_confidence(annotation: Option<&CalcitTypeAnnotation>) -> &'static str {
  let Some(annotation) = annotation else {
    return "unknown";
  };
  if matches!(annotation, CalcitTypeAnnotation::Dynamic) {
    return "unknown";
  }
  let rendered = annotation.to_brief_string();
  if rendered.contains("dynamic") || matches!(annotation, CalcitTypeAnnotation::DynFn | CalcitTypeAnnotation::Custom(_)) {
    "partial"
  } else if annotation.contains_type_var() {
    "generic"
  } else {
    "exact"
  }
}

fn warning_matches_type_at(warning: &LocatedWarning, namespace: &str, definition: &str, path: &[usize]) -> bool {
  let location = warning.location();
  if location.ns.as_ref() != namespace || location.def.as_ref() != definition {
    return false;
  }
  let warning_path = location.coord.iter().map(|idx| *idx as usize).collect::<Vec<_>>();
  warning_path.starts_with(path) || path.starts_with(&warning_path)
}

fn warning_to_context_diagnostic(warning: &LocatedWarning) -> ContextDiagnostic {
  ContextDiagnostic {
    code: warning.code().unwrap_or("W_PREPROCESS").to_owned(),
    phase: "type-check",
    severity: "warning",
    message: warning.message().to_owned(),
    path: Some(semantic_code_path(
      &warning.location().coord.iter().map(|idx| *idx as usize).collect::<Vec<_>>(),
    )),
    intent: None,
  }
}

fn type_at_expected_mismatch_diagnostic(
  actual: &CalcitTypeAnnotation,
  required: &CalcitTypeAnnotation,
  source: &str,
  path: &str,
) -> Option<ContextDiagnostic> {
  if actual.matches_annotation(required) {
    return None;
  }
  Some(ContextDiagnostic {
    code: "W_TYPE_AT_EXPECTED_MISMATCH".to_owned(),
    phase: "type-check",
    severity: "warning",
    message: format!(
      "Inferred type `{}` does not satisfy expected type `{}` from {source}",
      actual.to_brief_string(),
      required.to_brief_string()
    ),
    path: Some(path.to_owned()),
    intent: None,
  })
}

fn render_type_at_human(envelope: &SemanticQueryEnvelope<TypeAtData>) -> String {
  let data = &envelope.data;
  let mut out = String::new();
  let _ = writeln!(&mut out, "Definition: {}", data.id);
  let _ = writeln!(&mut out, "Path: {}", data.path);
  let _ = writeln!(&mut out, "Revision: {}", envelope.revision);
  let _ = writeln!(&mut out, "Expression: {}", data.expression);
  let _ = writeln!(&mut out, "Inferred type: {}", data.inferred_type.as_deref().unwrap_or("unknown"));
  if let Some(expected) = &data.expected_type {
    let _ = writeln!(
      &mut out,
      "Expected type: {} ({})",
      expected,
      data.expected_from.as_deref().unwrap_or("static context")
    );
  }
  let _ = writeln!(&mut out, "Confidence: {}", data.confidence);
  if let Some(intent) = data.dynamic_intent {
    let _ = writeln!(&mut out, "Dynamic intent: {intent}");
  }
  let _ = writeln!(&mut out, "Evidence:");
  for evidence in &data.evidence {
    let _ = writeln!(&mut out, "  - {}: {}", evidence.kind, evidence.detail);
  }
  let _ = writeln!(
    &mut out,
    "Bindings: {} (referenced/top-level; not a complete scope dump)",
    data.bindings.len()
  );
  for binding in &data.bindings {
    let location = binding.path.as_deref().map(|path| format!(" @ {path}")).unwrap_or_default();
    let _ = writeln!(&mut out, "  - {}: {}{}", binding.name, binding.r#type, location);
  }
  let _ = writeln!(&mut out, "Static methods:");
  match &data.static_methods {
    Some(methods) => {
      for method in methods {
        let _ = writeln!(&mut out, "  - {} ({})", method.name, method.origin);
      }
    }
    None => {
      let _ = writeln!(&mut out, "  unknown");
    }
  }
  let _ = writeln!(&mut out, "Diagnostics: {}", envelope.diagnostics.len());
  for diagnostic in &envelope.diagnostics {
    let location = diagnostic.path.as_deref().map(|path| format!(" @ {path}")).unwrap_or_default();
    let _ = writeln!(
      &mut out,
      "  - {} {}{}: {}",
      diagnostic.severity, diagnostic.code, location, diagnostic.message
    );
  }
  if !envelope.next.is_empty() {
    let _ = writeln!(&mut out, "Next:");
    for command in &envelope.next {
      let _ = writeln!(&mut out, "  - {command}");
    }
  }
  out
}

fn handle_type_at(input_path: &str, opts: &QueryTypeAtCommand) -> Result<(), String> {
  let format = parse_query_render_format(&opts.format)?;
  let target_path = parse_type_at_path(&opts.path)?;
  let (namespace, requested_definition) = parse_target(&opts.target)?;
  let resolved_input = calcit::resolve_snapshot_path_alias(Path::new(input_path));
  let snapshot = if resolved_input.exists() {
    load_snapshot(input_path)?
  } else if namespace == calcit::calcit::CORE_NS || namespace == "calcit.internal" {
    load_core_snapshot()?
  } else {
    return Err(format!("{} does not exist", resolved_input.display()));
  };
  let file = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace `{namespace}` not found"))?;
  let lookup = resolve_definition_lookup(namespace, requested_definition, file.defs.keys().map(String::as_str), true)?;
  if let Some(warning) = lookup.warning.as_deref() {
    print_cli_warning_block(warning);
  }
  let definition = lookup.resolved;
  let entry = file.defs.get(&definition).expect("resolved definition exists");
  let target_node = navigate_to_path(&entry.code, &target_path)?;
  let semantic_path = semantic_code_path(&target_path);
  let expression = format_cirru_expression(&target_node);
  let revision = snapshot::definition_revision(entry)?;

  prepare_program_for_type_query(&snapshot)?;
  let warnings = RefCell::<Vec<LocatedWarning>>::new(vec![]);
  let compile_error =
    runner::preprocess::compile_source_def_for_snapshot(namespace, &definition, &warnings, &CallStackList::default()).err();
  let compiled = program::lookup_compiled_def(namespace, &definition);
  let processed_root = compiled.as_ref().map(|compiled| &compiled.preprocessed_code);
  let processed_target = processed_root
    .and_then(|root| find_preprocessed_node_at_path(root, namespace, &definition, &target_path, matches!(target_node, Cirru::List(_))));
  let source_target = code_to_calcit(
    &target_node,
    namespace,
    &definition,
    target_path
      .iter()
      .map(|idx| u16::try_from(*idx).map_err(|_| format!("Path index {idx} exceeds Snapshot coordinate range")))
      .collect::<Result<Vec<_>, _>>()?,
  )?;
  let inference_target = processed_target.unwrap_or(&source_target);
  let inferred = runner::preprocess::infer_static_type_from_expr(inference_target);
  let expected = expected_type_at_path(entry, processed_root, namespace, &definition, &target_path);
  let inferred_rendered = inferred
    .as_ref()
    .map(|annotation| format_type_query_annotation(annotation.as_ref()))
    .transpose()?;
  let (expected_rendered, expected_from) = match expected.as_ref() {
    Some((annotation, source)) => (Some(format_type_query_annotation(annotation.as_ref())?), Some(source.clone())),
    None => (None, None),
  };
  let methods = inferred.as_ref().and_then(|annotation| {
    runner::preprocess::static_method_descriptors(annotation.as_ref()).map(|items| {
      items
        .into_iter()
        .map(|method| ContextMethod {
          name: method.name,
          origin: method.origin,
        })
        .collect::<Vec<_>>()
    })
  });

  let fn_features = context_features(entry.schema.as_ref());
  let dynamic_intent = if inferred
    .as_ref()
    .is_some_and(|annotation| matches!(annotation.as_ref(), CalcitTypeAnnotation::Dynamic))
    && fn_features.iter().any(|feature| feature == "js-ffi")
  {
    Some("intentional-js-ffi")
  } else if inferred
    .as_ref()
    .is_some_and(|annotation| matches!(annotation.as_ref(), CalcitTypeAnnotation::Dynamic))
  {
    Some("unresolved")
  } else {
    None
  };

  let mut diagnostics = warnings
    .borrow()
    .iter()
    .filter(|warning| warning_matches_type_at(warning, namespace, &definition, &target_path))
    .map(warning_to_context_diagnostic)
    .collect::<Vec<_>>();
  if let Some(error) = compile_error {
    diagnostics.push(ContextDiagnostic {
      code: error.code.unwrap_or_else(|| format!("E_{}", error.kind.to_string().to_uppercase())),
      phase: "preprocess",
      severity: "error",
      message: error.msg,
      path: error
        .location
        .as_ref()
        .map(|location| semantic_code_path(&location.coord.iter().map(|idx| *idx as usize).collect::<Vec<_>>())),
      intent: None,
    });
  }
  if inferred.is_none() {
    diagnostics.push(ContextDiagnostic {
      code: "W_TYPE_AT_UNRESOLVED".to_owned(),
      phase: "type-inference",
      severity: "warning",
      message: "Static inference could not determine a type for this expression without executing the program".to_owned(),
      path: Some(semantic_path.clone()),
      intent: dynamic_intent.map(str::to_owned),
    });
  }
  if let (Some(actual), Some((required, source))) = (inferred.as_ref(), expected.as_ref())
    && let Some(diagnostic) = type_at_expected_mismatch_diagnostic(actual.as_ref(), required.as_ref(), source, &semantic_path)
  {
    diagnostics.push(diagnostic);
  }

  let mut evidence = vec![type_at_evidence(inference_target, &semantic_path, processed_target.is_some())];
  if let Some(source) = &expected_from {
    evidence.push(TypeAtEvidence {
      kind: "expected-context".to_owned(),
      detail: source.clone(),
      path: target_path
        .split_last()
        .map(|(_, parent)| semantic_code_path(parent))
        .or_else(|| Some("code".to_owned())),
    });
  }
  let bindings = format_type_at_bindings(processed_root, processed_target)?;
  let confidence = type_at_confidence(inferred.as_deref());
  let data = TypeAtData {
    id: format!("{namespace}/{definition}"),
    path: semantic_path.clone(),
    expression,
    tree: cirru_to_json_value(&target_node),
    inferred_type: inferred_rendered,
    expected_type: expected_rendered,
    expected_from,
    confidence,
    dynamic_intent,
    evidence,
    bindings,
    bindings_complete: false,
    static_methods: methods,
  };
  let mut next = vec![format!("calcit tree show '{namespace}/{definition}' --path '{semantic_path}'")];
  if confidence != "exact" {
    next.push(format!("calcit query schema '{namespace}/{definition}'"));
    next.push(format!(
      "calcit analyze weak-types --ns '{namespace}' --intent unresolved --format json"
    ));
  }
  let envelope = SemanticQueryEnvelope {
    schema_version: 1,
    command: "query.type-at",
    revision,
    data,
    diagnostics,
    next,
  };

  match format {
    QueryRenderFormat::Human => print!("{}", render_type_at_human(&envelope)),
    QueryRenderFormat::Json => println!(
      "{}",
      serde_json::to_string_pretty(&envelope).map_err(|error| format!("Failed to encode type-at result: {error}"))?
    ),
  }
  Ok(())
}

fn truncate_chars(text: &str, max_chars: usize) -> (String, bool) {
  if max_chars == 0 {
    return (String::new(), !text.is_empty());
  }
  let Some((byte_index, _)) = text.char_indices().nth(max_chars) else {
    return (text.to_owned(), false);
  };
  (format!("{}…", &text[..byte_index]), true)
}

fn count_cirru_nodes(node: &Cirru) -> usize {
  match node {
    Cirru::Leaf(_) => 1,
    Cirru::List(items) => 1 + items.iter().map(count_cirru_nodes).sum::<usize>(),
  }
}

fn namespace_source(snapshot: &snapshot::Snapshot, namespace: &str) -> String {
  if namespace == calcit::calcit::CORE_NS || namespace.starts_with("calcit.") || namespace.starts_with("calcit-test.") {
    "core".to_owned()
  } else if namespace == snapshot.package || namespace.starts_with(&format!("{}.", snapshot.package)) {
    "project".to_owned()
  } else {
    "dependency".to_owned()
  }
}

fn context_schema(annotation: &CalcitTypeAnnotation) -> Result<Option<String>, String> {
  match annotation {
    CalcitTypeAnnotation::Dynamic => Ok(None),
    CalcitTypeAnnotation::Fn(_) => Ok(Some(format_query_schema(annotation, true).trim().to_owned())),
    _ => Ok(Some(format_type_query_annotation(annotation)?)),
  }
}

fn context_features(annotation: &CalcitTypeAnnotation) -> Vec<String> {
  let mut features = match annotation {
    CalcitTypeAnnotation::Fn(fn_annot) => fn_annot
      .features
      .iter()
      .map(|feature| feature.ref_str().to_owned())
      .collect::<Vec<_>>(),
    _ => vec![],
  };
  features.sort();
  features
}

fn build_context_code(node: &Cirru, max_chars: usize) -> Result<ContextCode, String> {
  let nodes = count_cirru_nodes(node);
  let rendered = cirru_parser::format(std::slice::from_ref(node), true.into())
    .map_err(|error| format!("Failed to format definition code: {error}"))?;
  let (cirru, truncated) = truncate_chars(rendered.trim(), max_chars);
  let tree = if !truncated && nodes <= 180 {
    Some(cirru_to_json(node))
  } else {
    None
  };
  Ok(ContextCode {
    root: "code",
    nodes,
    cirru,
    tree,
    truncated,
  })
}

fn build_context_examples(examples: &[Cirru], limit: usize, budget: usize) -> Result<ContextCollection<ContextExample>, String> {
  let returned_count = examples.len().min(limit);
  let per_example_budget = budget
    .checked_div(returned_count)
    .map(|value| value.clamp(120, 1200))
    .unwrap_or_default();
  let mut items = Vec::with_capacity(returned_count);

  for (index, example) in examples.iter().take(returned_count).enumerate() {
    let nodes = count_cirru_nodes(example);
    let rendered = cirru_parser::format(std::slice::from_ref(example), true.into())
      .map_err(|error| format!("Failed to format definition example {index}: {error}"))?;
    let (cirru, truncated) = truncate_chars(rendered.trim(), per_example_budget);
    items.push(ContextExample {
      index,
      cirru,
      tree: if !truncated && nodes <= 100 {
        Some(cirru_to_json(example))
      } else {
        None
      },
      truncated,
    });
  }

  Ok(ContextCollection::new(examples.len(), items))
}

fn build_context_tests(tests: &[snapshot::TestEntry], limit: usize, budget: usize) -> Result<ContextCollection<ContextTest>, String> {
  let returned_count = tests.len().min(limit);
  let per_test_budget = budget
    .checked_div(returned_count)
    .map(|value| value.clamp(120, 1200))
    .unwrap_or_default();
  let mut items = Vec::with_capacity(returned_count);
  for test in tests.iter().take(returned_count) {
    let nodes = count_cirru_nodes(&test.code);
    let rendered = cirru_parser::format(std::slice::from_ref(&test.code), true.into())
      .map_err(|error| format!("Failed to format definition test `{}`: {error}", test.name))?;
    let (cirru, truncated) = truncate_chars(rendered.trim(), per_test_budget);
    let mut tags = test.tags.iter().map(|tag| tag.ref_str().to_owned()).collect::<Vec<_>>();
    tags.sort();
    items.push(ContextTest {
      name: test.name.clone(),
      tags,
      cirru,
      tree: if !truncated && nodes <= 100 {
        Some(cirru_to_json(&test.code))
      } else {
        None
      },
      truncated,
    });
  }
  Ok(ContextCollection::new(tests.len(), items))
}

fn collect_direct_dependencies(
  snapshot: &snapshot::Snapshot,
  namespace: &str,
  definition: &str,
) -> Result<Vec<(String, String)>, String> {
  let mut analyzer = CallTreeAnalyzer::new(CallTreeConfig {
    include_core: true,
    max_depth: 1,
    show_unused: false,
    package_name: Some(snapshot.package.clone()),
    ns_prefix: None,
  });
  let result = analyzer.analyze(namespace, definition)?;
  let mut dependencies = result
    .tree
    .calls
    .into_iter()
    .map(|node| (node.fqn, node.source))
    .collect::<Vec<_>>();
  dependencies.sort();
  dependencies.dedup_by(|left, right| left.0 == right.0);
  Ok(dependencies)
}

fn context_usages(
  snapshot: &snapshot::Snapshot,
  namespace: &str,
  definition: &str,
  include_deps: bool,
  limit: usize,
) -> ContextCollection<ContextUsage> {
  let mut usages = collect_usages_from_snapshot(snapshot, namespace, definition, true)
    .into_iter()
    .filter_map(|(usage_ns, usage_def, _context, coords, area)| {
      let source = namespace_source(snapshot, &usage_ns);
      if !include_deps && source != "project" {
        return None;
      }
      Some(ContextUsage {
        id: format!("{usage_ns}/{usage_def}"),
        source,
        area: area.to_owned(),
        paths: coords.iter().map(|path| format!("{area}{}", format_path(path))).collect(),
      })
    })
    .collect::<Vec<_>>();
  usages.sort_by(|left, right| (&left.id, &left.area).cmp(&(&right.id, &right.area)));
  let total = usages.len();
  usages.truncate(limit);
  ContextCollection::new(total, usages)
}

fn context_docs(definition: &str, diagnostics: &mut Vec<ContextDiagnostic>) -> ContextCollection<ContextDocLink> {
  match super::docs::lookup_definition_docs(definition) {
    Ok(links) => {
      let total = links.len();
      let items = links
        .into_iter()
        .take(6)
        .map(|link| ContextDocLink {
          id: link.id,
          path: link.path,
          title: link.title,
          summary: link.summary,
        })
        .collect();
      ContextCollection::new(total, items)
    }
    Err(error) => {
      diagnostics.push(ContextDiagnostic {
        code: "I_DOC_INDEX_UNAVAILABLE".to_owned(),
        phase: "documentation",
        severity: "info",
        message: error.lines().next().unwrap_or(&error).to_owned(),
        path: None,
        intent: None,
      });
      ContextCollection::new(0, vec![])
    }
  }
}

fn context_methods(annotation: &CalcitTypeAnnotation, budget: usize) -> Option<ContextCollection<ContextMethod>> {
  let methods = runner::preprocess::static_method_descriptors(annotation)?;
  let total = methods.len();
  let limit = (budget / 90).clamp(4, 80);
  let items = methods
    .into_iter()
    .take(limit)
    .map(|method| ContextMethod {
      name: method.name,
      origin: method.origin,
    })
    .collect();
  Some(ContextCollection::new(total, items))
}

fn weak_type_diagnostics(entry: &snapshot::CodeEntry, budget: usize) -> Vec<ContextDiagnostic> {
  let Some(row) =
    crate::type_coverage::analyze_weak_types_entry("context", "target", entry, &crate::type_coverage::WeakTypeKind::all())
  else {
    return vec![];
  };

  let limit = (budget / 220).clamp(4, 24);
  let total = row.occurrences.len();
  let mut diagnostics = row
    .occurrences
    .into_iter()
    .take(limit)
    .map(|occurrence| {
      let intentional = matches!(
        occurrence.intent,
        crate::type_coverage::WeakTypeIntent::IntentionalJsFfi | crate::type_coverage::WeakTypeIntent::IntentionalTypeSlotDynamic
      );
      ContextDiagnostic {
        code: if intentional {
          "I_DYNAMIC_TYPE_INTENTIONAL".to_owned()
        } else {
          "W_DYNAMIC_TYPE_UNRESOLVED".to_owned()
        },
        phase: "static-analysis",
        severity: if intentional { "info" } else { "warning" },
        message: format!("{} ({})", occurrence.kind.as_str(), occurrence.detail),
        path: Some(occurrence.path),
        intent: Some(occurrence.intent.as_str().to_owned()),
      }
    })
    .collect::<Vec<_>>();
  if diagnostics.len() < total {
    diagnostics.push(ContextDiagnostic {
      code: "I_DIAGNOSTICS_TRUNCATED".to_owned(),
      phase: "static-analysis",
      severity: "info",
      message: format!(
        "{} additional weak-type diagnostic(s) omitted by the context budget",
        total - diagnostics.len()
      ),
      path: None,
      intent: None,
    });
  }
  diagnostics
}

fn build_regular_context(
  snapshot: &snapshot::Snapshot,
  namespace: &str,
  definition: &str,
  entry: &snapshot::CodeEntry,
  opts: &QueryContextCommand,
) -> Result<SemanticQueryEnvelope<DefinitionContextData>, String> {
  let revision = snapshot::definition_revision(entry)?;
  let coverage = crate::type_coverage::analyze_code_entry(namespace, definition, entry);
  let intentional_ffi = context_features(entry.schema.as_ref()).iter().any(|feature| feature == "js-ffi");
  let mut diagnostics = weak_type_diagnostics(entry, opts.budget);
  if coverage.level == crate::type_coverage::CoverageLevel::None && !intentional_ffi {
    diagnostics.insert(
      0,
      ContextDiagnostic {
        code: "W_TYPE_COVERAGE_NONE".to_owned(),
        phase: "static-analysis",
        severity: "warning",
        message: "definition has no trusted static type coverage".to_owned(),
        path: Some("schema".to_owned()),
        intent: None,
      },
    );
  }
  for issue in &coverage.schema_issues {
    diagnostics.push(ContextDiagnostic {
      code: "W_SCHEMA_MISMATCH".to_owned(),
      phase: "static-analysis",
      severity: "warning",
      message: issue.clone(),
      path: Some("schema".to_owned()),
      intent: None,
    });
  }

  let metadata_ready = prepare_program_for_type_query(snapshot);
  if let Err(error) = &metadata_ready {
    diagnostics.push(ContextDiagnostic {
      code: "W_STATIC_METADATA_UNAVAILABLE".to_owned(),
      phase: "preprocess",
      severity: "warning",
      message: error.clone(),
      path: None,
      intent: None,
    });
  }

  let dependencies = if metadata_ready.is_ok() {
    match collect_direct_dependencies(snapshot, namespace, definition) {
      Ok(items) => items,
      Err(error) => {
        diagnostics.push(ContextDiagnostic {
          code: "W_DEPENDENCY_ANALYSIS_FAILED".to_owned(),
          phase: "static-analysis",
          severity: "warning",
          message: error,
          path: Some("code".to_owned()),
          intent: None,
        });
        vec![]
      }
    }
  } else {
    vec![]
  };
  let dependency_total = dependencies.len();
  let dependency_items = dependencies
    .into_iter()
    .take(opts.dependency_limit)
    .map(|(id, source)| ContextDependency { id, source })
    .collect();

  let mut tags = entry.tags.iter().map(|tag| tag.ref_str().to_owned()).collect::<Vec<_>>();
  tags.sort();
  let doc_limit = (opts.budget / 6).clamp(160, 1200);
  let (doc, doc_truncated) = if entry.doc.trim().is_empty() {
    (None, false)
  } else {
    let (text, truncated) = truncate_chars(entry.doc.trim(), doc_limit);
    (Some(text), truncated)
  };
  let code = build_context_code(&entry.code, (opts.budget / 3).clamp(320, 2600))?;
  let examples = build_context_examples(&entry.examples, opts.example_limit, opts.budget / 4)?;
  let tests = build_context_tests(&entry.tests, opts.test_limit, opts.budget / 4)?;
  let usages = context_usages(snapshot, namespace, definition, opts.deps, opts.usage_limit);
  let id = format!("{namespace}/{definition}");
  let docs = context_docs(&id, &mut diagnostics);
  let static_methods = if metadata_ready.is_ok() {
    context_methods(entry.schema.as_ref(), opts.budget / 3)
  } else {
    None
  };

  let data = DefinitionContextData {
    id,
    uri: format!("calcit://definition/{namespace}/{definition}"),
    source: namespace_source(snapshot, namespace),
    kind: coverage.kind.as_str().to_owned(),
    coverage: if intentional_ffi && coverage.level == crate::type_coverage::CoverageLevel::None {
      "intentional-dynamic"
    } else {
      coverage.level.as_str()
    }
    .to_owned(),
    doc,
    doc_truncated,
    tags,
    schema: context_schema(entry.schema.as_ref())?,
    features: context_features(entry.schema.as_ref()),
    code,
    examples,
    tests,
    dependencies: ContextCollection::new(dependency_total, dependency_items),
    usages,
    docs,
    static_methods,
  };

  let mut next = vec![
    format!("calcit query schema {namespace}/{definition}"),
    format!("calcit docs graph explain {namespace}/{definition} --full"),
  ];
  if data.code.truncated || data.code.tree.is_none() {
    next.push(format!("calcit query def {namespace}/{definition}"));
  }
  if data.examples.truncated {
    next.push(format!("calcit query examples {namespace}/{definition}"));
  }
  if data.tests.truncated {
    next.push(format!("calcit query tests {namespace}/{definition}"));
  }
  if data.usages.truncated {
    next.push(format!("calcit query usages {namespace}/{definition}"));
  }

  Ok(SemanticQueryEnvelope {
    schema_version: 1,
    command: "query.context",
    revision,
    data,
    diagnostics,
    next,
  })
}

fn build_special_builtin_context(
  snapshot: &snapshot::Snapshot,
  namespace: &str,
  definition: &str,
  meta: SpecialBuiltinQueryMeta,
  opts: &QueryContextCommand,
) -> Result<SemanticQueryEnvelope<DefinitionContextData>, String> {
  let schema = format_query_schema(meta.schema.as_ref(), true).trim().to_owned();
  let examples_fingerprint = meta
    .examples
    .iter()
    .map(|example| example.format_one_liner().unwrap_or_default())
    .collect::<Vec<_>>()
    .join("\n");
  let revision = semantic_revision(&[meta.doc, &schema, &examples_fingerprint, meta.cirru_note]);
  let tags = meta.semantic_tags.iter().map(|tag| (*tag).to_owned()).collect::<Vec<_>>();
  let intentional_ffi = meta.semantic_tags.contains(&"js-ffi");
  let (doc, doc_truncated) = truncate_chars(meta.doc, (opts.budget / 6).clamp(160, 1200));
  let usages = context_usages(snapshot, namespace, definition, opts.deps, opts.usage_limit);
  let examples = build_context_examples(&meta.examples, opts.example_limit, opts.budget / 4)?;
  let id = format!("{namespace}/{definition}");
  let mut diagnostics = vec![];
  let docs = context_docs(&id, &mut diagnostics);

  let metadata_ready = prepare_program_for_type_query(snapshot).is_ok();
  let static_methods = if metadata_ready {
    context_methods(meta.schema.as_ref(), opts.budget / 3)
  } else {
    None
  };
  let data = DefinitionContextData {
    id,
    uri: format!("calcit://definition/{namespace}/{definition}"),
    source: "core-builtin".to_owned(),
    kind: "proc".to_owned(),
    coverage: if intentional_ffi { "intentional-dynamic" } else { "partial" }.to_owned(),
    doc: Some(doc),
    doc_truncated,
    tags: tags.clone(),
    schema: Some(schema),
    features: tags,
    code: ContextCode {
      root: "code",
      nodes: 0,
      cirru: meta.cirru_note.to_owned(),
      tree: None,
      truncated: false,
    },
    examples,
    tests: ContextCollection::new(0, vec![]),
    dependencies: ContextCollection::new(0, vec![]),
    usages,
    docs,
    static_methods,
  };
  if intentional_ffi {
    diagnostics.insert(
      0,
      ContextDiagnostic {
        code: "I_DYNAMIC_TYPE_INTENTIONAL".to_owned(),
        phase: "static-analysis",
        severity: "info",
        message: "dynamic values are intentional at this JS FFI boundary".to_owned(),
        path: Some("schema".to_owned()),
        intent: Some("intentional-js-ffi".to_owned()),
      },
    );
  }

  Ok(SemanticQueryEnvelope {
    schema_version: 1,
    command: "query.context",
    revision,
    data,
    diagnostics,
    next: vec![
      format!("calcit query def {namespace}/{definition}"),
      format!("calcit query examples {namespace}/{definition}"),
    ],
  })
}

fn render_context_human(envelope: &SemanticQueryEnvelope<DefinitionContextData>) -> String {
  let data = &envelope.data;
  let mut out = String::new();
  let _ = writeln!(&mut out, "Definition: {}", data.id);
  let _ = writeln!(&mut out, "Resource: {}", data.uri);
  let _ = writeln!(&mut out, "Revision: {}", envelope.revision);
  let _ = writeln!(&mut out, "Source: {}", data.source);
  let _ = writeln!(&mut out, "Kind: {}", data.kind);
  let _ = writeln!(&mut out, "Type coverage: {}", data.coverage);
  let _ = writeln!(
    &mut out,
    "Tags: {}",
    if data.tags.is_empty() {
      "-".to_owned()
    } else {
      data.tags.iter().map(|tag| format!(":{tag}")).collect::<Vec<_>>().join(" ")
    }
  );
  let _ = writeln!(
    &mut out,
    "Features: {}",
    if data.features.is_empty() {
      "-".to_owned()
    } else {
      data
        .features
        .iter()
        .map(|feature| format!(":{feature}"))
        .collect::<Vec<_>>()
        .join(" ")
    }
  );

  let _ = writeln!(&mut out, "\nDoc:");
  if let Some(doc) = &data.doc {
    for line in doc.lines() {
      let _ = writeln!(&mut out, "  {line}");
    }
    if data.doc_truncated {
      let _ = writeln!(&mut out, "  (truncated)");
    }
  } else {
    let _ = writeln!(&mut out, "  -");
  }

  let _ = writeln!(&mut out, "\nSchema:");
  if let Some(schema) = &data.schema {
    for line in schema.lines() {
      let _ = writeln!(&mut out, "  {line}");
    }
  } else {
    let _ = writeln!(&mut out, "  :dynamic (no explicit schema)");
  }

  let _ = writeln!(
    &mut out,
    "\nCode preview: {} node(s){}",
    data.code.nodes,
    if data.code.truncated { " (truncated)" } else { "" }
  );
  for line in data.code.cirru.lines() {
    let _ = writeln!(&mut out, "  {line}");
  }

  let _ = writeln!(
    &mut out,
    "\nExamples: {}/{}{}",
    data.examples.returned,
    data.examples.total,
    if data.examples.truncated { " (truncated)" } else { "" }
  );
  for example in &data.examples.items {
    let _ = writeln!(
      &mut out,
      "  [{}]{}",
      example.index,
      if example.truncated { " (truncated)" } else { "" }
    );
    for line in example.cirru.lines() {
      let _ = writeln!(&mut out, "    {line}");
    }
  }

  let _ = writeln!(
    &mut out,
    "\nTests: {}/{}{}",
    data.tests.returned,
    data.tests.total,
    if data.tests.truncated { " (truncated)" } else { "" }
  );
  for test in &data.tests.items {
    let tags = if test.tags.is_empty() {
      String::new()
    } else {
      format!(" [{}]", test.tags.iter().map(|tag| format!(":{tag}")).collect::<Vec<_>>().join(" "))
    };
    let _ = writeln!(
      &mut out,
      "  # {}{}{}",
      test.name,
      tags,
      if test.truncated { " (truncated)" } else { "" }
    );
    for line in test.cirru.lines() {
      let _ = writeln!(&mut out, "    {line}");
    }
  }

  let _ = writeln!(
    &mut out,
    "\nDirect dependencies: {}/{}{}",
    data.dependencies.returned,
    data.dependencies.total,
    if data.dependencies.truncated { " (truncated)" } else { "" }
  );
  for dependency in &data.dependencies.items {
    let _ = writeln!(&mut out, "  - {} [{}]", dependency.id, dependency.source);
  }

  let _ = writeln!(
    &mut out,
    "\nUsages: {}/{}{}",
    data.usages.returned,
    data.usages.total,
    if data.usages.truncated { " (truncated)" } else { "" }
  );
  for usage in &data.usages.items {
    let paths = if usage.paths.is_empty() {
      "-".to_owned()
    } else {
      usage.paths.join(", ")
    };
    let _ = writeln!(&mut out, "  - {} [{}:{}] @ {}", usage.id, usage.source, usage.area, paths);
  }

  let _ = writeln!(
    &mut out,
    "\nRelated docs: {}/{}{}",
    data.docs.returned,
    data.docs.total,
    if data.docs.truncated { " (truncated)" } else { "" }
  );
  for doc in &data.docs.items {
    let title = doc.title.as_deref().unwrap_or("untitled");
    let summary = doc.summary.as_deref().map(|summary| format!(" — {summary}")).unwrap_or_default();
    let _ = writeln!(&mut out, "  - {}: {} ({}){}", doc.id, title, doc.path, summary);
  }

  let _ = writeln!(&mut out, "\nStatic methods:");
  match &data.static_methods {
    Some(methods) => {
      let _ = writeln!(
        &mut out,
        "  {}/{}{}",
        methods.returned,
        methods.total,
        if methods.truncated { " (truncated)" } else { "" }
      );
      for method in &methods.items {
        let _ = writeln!(&mut out, "  - {} ({})", method.name, method.origin);
      }
    }
    None => {
      let _ = writeln!(&mut out, "  unknown");
    }
  }

  let _ = writeln!(&mut out, "\nDiagnostics: {}", envelope.diagnostics.len());
  for diagnostic in &envelope.diagnostics {
    let location = diagnostic.path.as_deref().map(|path| format!(" @ {path}")).unwrap_or_default();
    let intent = diagnostic
      .intent
      .as_deref()
      .map(|intent| format!(" [{intent}]"))
      .unwrap_or_default();
    let _ = writeln!(
      &mut out,
      "  - {} {}{}{}: {}",
      diagnostic.severity, diagnostic.code, intent, location, diagnostic.message
    );
  }

  if !envelope.next.is_empty() {
    let _ = writeln!(&mut out, "\nNext:");
    for command in &envelope.next {
      let _ = writeln!(&mut out, "  - {command}");
    }
  }
  out
}

fn handle_context(input_path: &str, opts: &QueryContextCommand) -> Result<(), String> {
  if opts.budget < 512 {
    return Err("Context budget must be at least 512 characters".to_owned());
  }
  let format = parse_query_render_format(&opts.format)?;
  let (namespace, requested_definition) = parse_target(&opts.target)?;
  let resolved_input = calcit::resolve_snapshot_path_alias(Path::new(input_path));
  let snapshot = if resolved_input.exists() {
    load_snapshot(input_path)?
  } else if namespace == calcit::calcit::CORE_NS || namespace == "calcit.internal" {
    load_core_snapshot()?
  } else {
    return Err(format!("{} does not exist", resolved_input.display()));
  };
  let file = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace `{namespace}` not found"))?;

  let envelope = if !file.defs.contains_key(requested_definition) {
    if let Some(meta) = lookup_special_builtin_query_meta(namespace, requested_definition)? {
      build_special_builtin_context(&snapshot, namespace, requested_definition, meta, opts)?
    } else {
      let lookup = resolve_definition_lookup(namespace, requested_definition, file.defs.keys().map(|name| name.as_str()), true)?;
      if let Some(warning) = lookup.warning.as_deref() {
        print_cli_warning_block(warning);
      }
      let entry = file.defs.get(&lookup.resolved).expect("resolved definition exists");
      build_regular_context(&snapshot, namespace, &lookup.resolved, entry, opts)?
    }
  } else {
    let entry = file.defs.get(requested_definition).expect("checked definition exists");
    build_regular_context(&snapshot, namespace, requested_definition, entry, opts)?
  };

  match format {
    QueryRenderFormat::Human => print!("{}", render_context_human(&envelope)),
    QueryRenderFormat::Json => println!(
      "{}",
      serde_json::to_string_pretty(&envelope).map_err(|error| format!("Failed to encode context query result: {error}"))?
    ),
  }
  Ok(())
}

/// Load a module silently (without println)
fn load_module_silent(path: &str, base_dir: &Path, module_folder: &Path) -> Result<snapshot::Snapshot, String> {
  let previous = calcit::quiet_tool_output();
  calcit::set_quiet_tool_output(true);
  let result = calcit::load_module(path, base_dir, module_folder);
  calcit::set_quiet_tool_output(previous);
  result
}

fn load_snapshot(input_path: &str) -> Result<snapshot::Snapshot, String> {
  load_snapshot_with_entry(input_path, None)
}

fn load_main_snapshot(input_path: &str) -> Result<snapshot::Snapshot, String> {
  let resolved_input_path = calcit::resolve_snapshot_path_alias(Path::new(input_path));
  let resolved_input_str = resolved_input_path.to_string_lossy().to_string();
  if !resolved_input_path.exists() {
    return Err(format!("{} does not exist", resolved_input_path.display()));
  }

  let mut content = fs::read_to_string(&resolved_input_path).map_err(|e| format!("Failed to read file: {e}"))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content).map_err(|e| {
    eprintln!("\nFailed to parse file '{}':", resolved_input_path.display());
    eprintln!("{e}");
    format!("Failed to parse file '{}'", resolved_input_path.display())
  })?;
  let snapshot = snapshot::load_snapshot_data(&data, &resolved_input_str)?;
  let project_namespaces = snapshot.files.keys().cloned().collect::<HashSet<_>>();
  runner::preprocess::set_project_namespaces(&project_namespaces);
  Ok(snapshot)
}

/// Prefer the project Snapshot for metadata-only queries. Dependencies and
/// core are loaded only when the requested namespace is not local.
fn load_snapshot_for_namespace(input_path: &str, namespace: &str) -> Result<snapshot::Snapshot, String> {
  let main_snapshot = load_main_snapshot(input_path)?;
  if main_snapshot.files.contains_key(namespace) {
    return Ok(main_snapshot);
  }
  if namespace == calcit::calcit::CORE_NS || namespace == "calcit.internal" {
    return load_core_snapshot();
  }
  load_snapshot(input_path)
}

fn load_snapshot_for_search(input_path: &str, options: &SearchCommonOpts) -> Result<snapshot::Snapshot, String> {
  if let Some(entry) = options.entry {
    return load_snapshot_with_entry(input_path, Some(entry));
  }
  if let Some(filter) = options.filter {
    let namespace = filter.split_once('/').map(|(namespace, _)| namespace).unwrap_or(filter);
    return load_snapshot_for_namespace(input_path, namespace);
  }
  load_snapshot(input_path)
}

pub(crate) fn load_snapshot_for_static_analysis(input_path: &str) -> Result<snapshot::Snapshot, String> {
  load_snapshot(input_path)
}

fn load_snapshot_with_entry(input_path: &str, entry: Option<&str>) -> Result<snapshot::Snapshot, String> {
  let mut snapshot = load_main_snapshot(input_path)?;
  snapshot.select_entry(entry)?;
  let mut modules_to_load = snapshot.active_entry()?.modules.clone();

  let mut seen_modules = HashSet::new();
  modules_to_load.retain(|module_path| seen_modules.insert(module_path.to_owned()));

  // Load modules (dependencies) silently
  let base_dir = Path::new(input_path).parent().unwrap_or(Path::new("."));
  let module_folder = calcit::project_module_folder(base_dir);

  for module_path in &modules_to_load {
    match load_module_silent(module_path, base_dir, &module_folder) {
      Ok(module_snapshot) => {
        calcit::merge_module_files(&mut snapshot, &module_snapshot, module_path)?;
      }
      Err(e) => {
        eprintln!("Warning: Failed to load module '{module_path}': {e}");
      }
    }
  }

  // Merge calcit.core definitions from built-in calcit-core.cirru
  let core_snapshot = load_core_snapshot()?;
  for (ns_name, file_data) in core_snapshot.files {
    snapshot.files.entry(ns_name).or_insert(file_data);
  }

  Ok(snapshot)
}

/// Handle `query ns` - list namespaces or show ns details
fn handle_ns(input_path: &str, namespace: Option<&str>, include_deps: bool) -> Result<(), String> {
  // If namespace is provided, show details (merged read-ns functionality)
  if let Some(ns_name) = namespace {
    return handle_ns_details(input_path, ns_name);
  }

  // Otherwise list all namespaces
  if !Path::new(input_path).exists() {
    return Err(format!("{input_path} does not exist"));
  }

  let mut content = fs::read_to_string(input_path).map_err(|e| format!("Failed to read file: {e}"))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content).map_err(|e| {
    eprintln!("\nFailed to parse file '{input_path}':");
    eprintln!("{e}");
    format!("Failed to parse file '{input_path}'")
  })?;
  let main_snapshot = snapshot::load_snapshot_data(&data, input_path)?;
  let main_package = main_snapshot.package.clone();

  let snapshot = if include_deps { load_snapshot(input_path)? } else { main_snapshot };

  let mut namespaces: Vec<&String> = snapshot.files.keys().collect();
  namespaces.sort();

  let filtered: Vec<_> = namespaces
    .iter()
    .filter(|ns| {
      if !include_deps {
        ns.as_str() == main_package || ns.starts_with(&format!("{main_package}."))
      } else {
        true
      }
    })
    .collect();

  println!(
    "{} ({} namespaces)",
    if include_deps { "All namespaces:" } else { "Project namespaces:" }.bold(),
    filtered.len()
  );

  for ns in &filtered {
    println!("  {}", ns.cyan());
  }

  Ok(())
}

/// Handle `query ns <namespace>` - show ns details
fn handle_ns_details(input_path: &str, namespace: &str) -> Result<(), String> {
  let snapshot = load_snapshot_for_namespace(input_path, namespace)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  if !file_data.ns.doc.is_empty() {
    println!("{} {}", "Doc:".bold(), file_data.ns.doc);
  }

  println!("\n{}", "NS declaration:".bold());
  let ns_str =
    cirru_parser::format(std::slice::from_ref(&file_data.ns.code), true.into()).unwrap_or_else(|_| "(failed to format)".to_string());
  println!("{}", ns_str.dimmed());

  println!("\n{} {}", "Definitions:".bold(), file_data.defs.len());

  Ok(())
}

fn parse_query_tag(raw: &str) -> Result<EdnTag, String> {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return Err("empty tag".to_string());
  }
  let name = trimmed.strip_prefix(':').unwrap_or(trimmed);
  if name.is_empty() {
    return Err(format!("invalid tag: {raw}"));
  }
  Ok(EdnTag::new(name))
}

fn format_tags_display(tags: &HashSet<EdnTag>) -> String {
  let mut items: Vec<String> = tags.iter().map(|tag| format!(":{}", tag.ref_str())).collect();
  items.sort();
  items.join(",")
}

fn handle_host_procs(opts: &QueryHostProcsCommand) -> Result<(), String> {
  let filter_tag = opts.tag.as_deref().map(parse_query_tag).transpose()?;
  let mut items = calcit::builtins::list_registered_procs();

  if let Some(tag) = &filter_tag {
    items.retain(|(_, descriptor)| descriptor.tags.contains(tag));
  }

  if let Some(tag) = &filter_tag {
    println!(
      "{} {} (filtered by {})",
      "Registered procs:".bold(),
      items.len(),
      format!(":{}", tag.ref_str()).yellow()
    );
  } else {
    println!("{} {}", "Registered procs:".bold(), items.len());
  }

  for (name, descriptor) in items {
    let tags = if descriptor.tags.is_empty() {
      "-".dimmed().to_string()
    } else {
      format_tags_display(&descriptor.tags)
    };
    println!("  {}  {}", name.cyan(), tags.dimmed());
  }

  Ok(())
}

fn handle_defs(input_path: &str, opts: &QueryDefsCommand) -> Result<(), String> {
  let namespace = &opts.namespace;
  let snapshot = load_snapshot_for_namespace(input_path, namespace)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let filter_tag = opts.tag.as_deref().map(parse_query_tag).transpose()?;

  let mut defs: Vec<&String> = file_data.defs.keys().collect();
  defs.sort();
  let total = defs.len();

  if let Some(tag) = &filter_tag {
    defs.retain(|def| file_data.defs[*def].tags.contains(tag));
  }

  if let Some(tag) = &filter_tag {
    println!(
      "{} {} (filtered by {}, {} total)",
      "Definitions:".bold(),
      defs.len(),
      format!(":{}", tag.ref_str()).yellow(),
      total
    );
  } else {
    println!("{} {}", "Definitions:".bold(), defs.len());
  }

  for def in &defs {
    let entry = &file_data.defs[*def];
    let tags_hint = if entry.tags.is_empty() {
      String::new()
    } else {
      format!(" [{}]", format_tags_display(&entry.tags))
    };
    let schema_hint = if !matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Dynamic) {
      " [schema]"
    } else {
      ""
    };
    if !entry.doc.is_empty() {
      let doc_first_line = entry.doc.lines().next().unwrap_or("");
      let doc_display = if doc_first_line.len() > 50 {
        format!("{}...", &doc_first_line[..50])
      } else {
        doc_first_line.to_string()
      };
      println!(
        "  {}{}{} - {}",
        def.green(),
        tags_hint.yellow(),
        schema_hint.dimmed(),
        doc_display.dimmed()
      );
    } else {
      println!("  {}{}{}", def.green(), tags_hint.yellow(), schema_hint.dimmed());
    }
  }

  Ok(())
}

fn handle_pkg(input_path: &str) -> Result<(), String> {
  let snapshot = load_main_snapshot(input_path)?;
  println!("{}", snapshot.package);
  Ok(())
}

fn handle_config(input_path: &str) -> Result<(), String> {
  let snapshot = load_main_snapshot(input_path)?;

  println!("{}", "Project Config:".bold());
  let deps_path = deps_path_for_snapshot(input_path);
  println!("  {}: managed in deps.cirru (use `caps version get {deps_path}`)", "version".cyan());
  println!("\n{}", "Snapshot Entries:".bold());

  let mut names: Vec<&String> = snapshot.entries.keys().collect();
  names.sort();

  for name in names {
    let entry = snapshot
      .entries
      .get(name)
      .ok_or_else(|| format!("Missing entry config for '{name}'"))?;

    println!("  {}", name.cyan());
    println!("    {}: {}", "mode".cyan(), entry.mode);
    println!("    {}: {}", "init_fn".cyan(), entry.init_fn);
    println!("    {}: {}", "reload_fn".cyan(), entry.reload_fn);
    println!("    {}: {:?}", "modules".cyan(), entry.modules);
    println!("    {}: {:?}", "type_slots".cyan(), entry.type_slots);
  }

  Ok(())
}

fn handle_error(input_path: &str) -> Result<(), String> {
  let project_directory = project_state::project_directory_for_snapshot(input_path);
  let error_file = project_state::state_file(project_directory, ERROR_STATE_FILE);
  let legacy_error_file = project_directory.join(".calcit-error.cirru");
  if project_state::migrate_legacy_file(&legacy_error_file, &error_file)
    .map_err(|error| format!("Failed to migrate legacy error file: {error}"))?
  {
    eprintln!("Moved legacy error state into '{}'.", error_file.display());
  }

  if !error_file.exists() {
    println!("{}", "No .calcit/error.cirru file found.".yellow());
    if command_guidance_enabled() {
      println!();
      println!("{}", "Next steps:".blue().bold());
      println!("  • Start watcher: {} or {}", "calcit".cyan(), "calcit js".cyan());
      println!("  • Run syntax check: {}", "calcit --check-only".cyan());
    }
    return Ok(());
  }

  let metadata = fs::metadata(&error_file).map_err(|e| format!("Failed to get metadata of error file: {e}"))?;
  if let Ok(modified) = metadata.modified()
    && let Ok(elapsed) = modified.elapsed()
    && elapsed.as_secs() > 10
  {
    println!(
      "{}",
      format!("Warning: .calcit/error.cirru was modified {} seconds ago.", elapsed.as_secs()).yellow()
    );
    println!("{}", "It might be outdated, please recompile or check the watcher.".yellow());
    println!();
  }

  let content = fs::read_to_string(&error_file).map_err(|e| format!("Failed to read error file: {e}"))?;

  if content.trim().is_empty() {
    println!("{}", "✓ Error file is empty (no recent errors).".green());
    println!();
    println!("{}", "Your code compiled successfully!".dimmed());
    println!(
      "{}",
      "Note: this only reflects recent Calcit parsing/preprocess/runtime status; still validate browser rendering, CSS values, and external side effects separately."
        .dimmed()
    );
  } else {
    println!("{}", "Last error stack trace:".bold().red());
    println!("{content}");
    if command_guidance_enabled() {
      println!();
      println!("{}", "Next steps to fix:".blue().bold());
      println!("  • Search for error location: {} '<symbol>'", "calcit query search".cyan());
      println!("  • View definition: {} '<ns/def>'", "calcit query def".cyan());
      println!("  • Find usages: {} '<ns/def>'", "calcit query usages".cyan());
      println!();
      println!("{}", "Tip: After fixing, watcher will recompile automatically (~300ms).".dimmed());
    }
    println!(
      "{}",
      "Note: even when this clears, non-Calcit issues like CSS strings, DOM behavior, and external integrations can still be wrong."
        .dimmed()
    );
  }

  Ok(())
}

fn handle_modules(input_path: &str) -> Result<(), String> {
  if !Path::new(input_path).exists() {
    return Err(format!("{input_path} does not exist"));
  }

  let mut content = fs::read_to_string(input_path).map_err(|e| format!("Failed to read file: {e}"))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content).map_err(|e| {
    eprintln!("\nFailed to parse file '{input_path}':");
    eprintln!("{e}");
    format!("Failed to parse file '{input_path}'")
  })?;
  let snapshot = snapshot::load_snapshot_data(&data, input_path)?;

  let base_dir = Path::new(input_path).parent().unwrap_or(Path::new("."));
  let module_folder = calcit::project_module_folder(base_dir);

  println!("{}", "Modules in project:".bold());

  println!("  {} {}", snapshot.package.cyan(), "(main)".dimmed());

  for module_path in &snapshot.active_entry()?.modules {
    match load_module_silent(module_path, base_dir, &module_folder) {
      Ok(module_snapshot) => {
        println!("  {} {}", module_snapshot.package.cyan(), format!("({module_path})").dimmed());
      }
      Err(_) => {
        println!("  {} {}", module_path.yellow(), "(failed)".red());
      }
    }
  }

  if !snapshot.entries.is_empty() {
    println!("\n{}", "Entries:".bold());
    for name in snapshot.entries.keys() {
      println!("  {}", name.cyan());
    }
  }

  Ok(())
}

fn render_chunked_display(display: &ChunkedDisplay) -> String {
  let mut out = String::new();
  let _ = writeln!(&mut out, "{}", "Chunked Cirru:".bold());
  let _ = writeln!(
    &mut out,
    "{}",
    format!(
      "nodes: {}, branches: {}, leaves: {}, max depth: {}, fragments: {}",
      display.total.nodes,
      display.total.branches,
      display.total.leaves,
      display.total.max_depth,
      display.fragments.len()
    )
    .dimmed()
  );
  let _ = writeln!(&mut out);

  for fragment in &display.fragments {
    let _ = writeln!(
      &mut out,
      "{} {}",
      fragment.id.cyan().bold(),
      format!("at {}", fragment.coord).dimmed()
    );
    let _ = writeln!(
      &mut out,
      "{}",
      format!("nodes: {}, max depth: {}", fragment.nodes, fragment.depth).dimmed()
    );
    for line in fragment.cirru.lines() {
      let _ = writeln!(&mut out, "  {line}");
    }
    let _ = writeln!(&mut out);
  }

  out
}

fn handle_def(input_path: &str, namespace: &str, definition: &str, opts: &QueryDefCommand) -> Result<(), String> {
  let snapshot = load_snapshot_for_namespace(input_path, namespace)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  if !file_data.defs.contains_key(definition)
    && let Some(meta) = lookup_special_builtin_query_meta(namespace, definition)?
  {
    let mut out = String::new();
    let _ = writeln!(&mut out, "{} {}", "Type:".bold(), meta.expr_preview);
    let _ = writeln!(&mut out, "{} {}", "Doc:".bold(), meta.doc);
    let _ = writeln!(&mut out, "\n{} {}", "Examples:".bold(), meta.examples.len());
    let _ = writeln!(&mut out, "\n{}", "Schema:".bold());
    let _ = writeln!(&mut out, "{}", format_query_schema(meta.schema.as_ref(), true));
    let _ = writeln!(&mut out, "\n{}", "Cirru:".bold());
    let _ = writeln!(&mut out, "{}", meta.cirru_note.dimmed());

    if opts.json {
      let _ = writeln!(&mut out, "\n{}", "JSON:".bold());
      let _ = writeln!(
        &mut out,
        "{}",
        serde_json::json!({
          "doc": meta.doc,
          "examples": meta.examples.iter().map(cirru_to_json).collect::<Vec<_>>(),
          "code": serde_json::Value::Null,
          "schema": cirru_to_json(&snapshot::schema_edn_to_cirru(
            &meta
              .schema
              .as_function()
              .map(|annot| annot.to_schema_edn())
              .unwrap_or(cirru_edn::Edn::Nil)
          ).unwrap_or_else(|_| Cirru::Leaf(Arc::from("nil")))),
          "builtin": true,
          "kind": "special-proc"
        })
      );
    }

    emit_cli_output(&out, false);
    return Ok(());
  }

  let lookup = resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), true)?;
  let render_to_stderr = lookup.warning.is_some();
  if let Some(warning) = lookup.warning.as_deref() {
    print_cli_warning_block(warning);
  }
  let resolved_definition = lookup.resolved;

  let code_entry = file_data
    .defs
    .get(resolved_definition.as_str())
    .expect("resolved definition exists");

  let mut out = String::new();

  if let Ok(code_data) = calcit::data::cirru::code_to_calcit(&code_entry.code, namespace, &resolved_definition, vec![])
    && let Some(summary) = CalcitTypeAnnotation::summarize_code(&code_data)
  {
    let _ = writeln!(&mut out, "{} {}", "Type:".bold(), summary);
  }

  if !code_entry.doc.is_empty() {
    let _ = writeln!(&mut out, "{} {}", "Doc:".bold(), code_entry.doc);
  }

  let tags_text = format_tags_display(&code_entry.tags);
  if tags_text.is_empty() {
    let _ = writeln!(&mut out, "{} {}", "Tags:".bold(), "(none)".dimmed());
  } else {
    let _ = writeln!(&mut out, "{} {}", "Tags:".bold(), tags_text);
  }

  if let Some(ffi) = &code_entry.ffi {
    let _ = writeln!(&mut out, "{} {}", "FFI:".bold(), format_edn_display(ffi));
  }

  if !code_entry.examples.is_empty() {
    let _ = writeln!(&mut out, "\n{} {}", "Examples:".bold(), code_entry.examples.len());
  }
  if !code_entry.tests.is_empty() {
    let _ = writeln!(&mut out, "{} {}", "Tests:".bold(), code_entry.tests.len());
  }

  let _ = writeln!(&mut out, "\n{}", "Schema:".bold());
  let schema_str = format_query_schema(code_entry.schema.as_ref(), true);
  let _ = writeln!(&mut out, "{schema_str}");

  if !opts.raw {
    let chunk_options = ChunkDisplayOptions {
      trigger_nodes: opts.chunk_trigger_nodes,
      target_nodes: opts.chunk_target_nodes,
      max_nodes: opts.chunk_max_nodes,
      max_branches: 64,
    };
    if let Some(display) = maybe_chunk_node(&code_entry.code, &chunk_options)? {
      let _ = writeln!(&mut out);
      out.push_str(&render_chunked_display(&display));
    } else {
      let _ = writeln!(&mut out, "\n{}", "Cirru:".bold());
      let cirru_str =
        cirru_parser::format(std::slice::from_ref(&code_entry.code), true.into()).unwrap_or_else(|_| "(failed to format)".to_string());
      let _ = writeln!(&mut out, "{cirru_str}");
    }
  } else {
    let _ = writeln!(&mut out, "\n{}", "Cirru:".bold());
    let cirru_str =
      cirru_parser::format(std::slice::from_ref(&code_entry.code), true.into()).unwrap_or_else(|_| "(failed to format)".to_string());
    let _ = writeln!(&mut out, "{cirru_str}");
  }

  if opts.json {
    let _ = writeln!(&mut out, "\n{}", "JSON:".bold());
    let json = code_entry_to_json(code_entry);
    let _ = writeln!(&mut out, "{}", serde_json::to_string(&json).unwrap());
  }

  emit_cli_output(&out, render_to_stderr);
  Ok(())
}

fn cirru_to_json(cirru: &Cirru) -> serde_json::Value {
  match cirru {
    Cirru::Leaf(s) => serde_json::Value::String(s.to_string()),
    Cirru::List(items) => serde_json::Value::Array(items.iter().map(cirru_to_json).collect()),
  }
}

fn code_entry_to_json(entry: &snapshot::CodeEntry) -> serde_json::Value {
  let schema_json = query_schema_cirru(entry.schema.as_ref(), false)
    .ok()
    .flatten()
    .map(|cirru| cirru_to_json(&cirru));
  let mut tags: Vec<String> = entry.tags.iter().map(|tag| tag.ref_str().to_string()).collect();
  tags.sort();
  serde_json::json!({
    "doc": entry.doc,
    "tags": tags,
    "examples": entry.examples.iter().map(cirru_to_json).collect::<Vec<_>>(),
    "tests": entry.tests.iter().map(|test| {
      let mut tags = test.tags.iter().map(|tag| tag.ref_str().to_owned()).collect::<Vec<_>>();
      tags.sort();
      serde_json::json!({"name": test.name, "tags": tags, "code": cirru_to_json(&test.code)})
    }).collect::<Vec<_>>(),
    "code": cirru_to_json(&entry.code),
    "schema": schema_json,
    "ffi": entry.ffi.as_ref().map(format_edn_display),
  })
}

fn format_example_node(example: &Cirru) -> String {
  match example {
    Cirru::Leaf(value) => value.to_string(),
    Cirru::List(_) => cirru_parser::format(std::slice::from_ref(example), true.into())
      .map(|text| text.trim().to_owned())
      .unwrap_or_else(|_| format!("{example:?}")),
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Progressive disclosure commands
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_examples(input_path: &str, namespace: &str, definition: &str) -> Result<(), String> {
  let snapshot = load_snapshot_for_namespace(input_path, namespace)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  if !file_data.defs.contains_key(definition)
    && let Some(meta) = lookup_special_builtin_query_meta(namespace, definition)?
  {
    let mut out = String::new();

    if meta.examples.is_empty() {
      let _ = writeln!(&mut out, "\n{}", "(no examples)".dimmed());
    } else {
      let _ = writeln!(&mut out, "{} example(s)\n", meta.examples.len());

      for (i, example) in meta.examples.iter().enumerate() {
        let _ = writeln!(&mut out, "{}", format!("[{i}]:").bold());
        let cirru_str = format_example_node(example);
        for line in cirru_str.lines().filter(|line| !line.trim().is_empty()) {
          let _ = writeln!(&mut out, "  {line}");
        }
        let _ = writeln!(
          &mut out,
          "  {} {}",
          "JSON:".dimmed(),
          serde_json::to_string(&cirru_to_json(example)).unwrap().dimmed()
        );
        let _ = writeln!(&mut out);
      }
    }

    emit_cli_output(&out, false);
    return Ok(());
  }

  let lookup = resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), true)?;
  let render_to_stderr = lookup.warning.is_some();
  if let Some(warning) = lookup.warning.as_deref() {
    print_cli_warning_block(warning);
  }
  let resolved_definition = lookup.resolved;

  let code_entry = file_data
    .defs
    .get(resolved_definition.as_str())
    .expect("resolved definition exists");

  let mut out = String::new();

  if code_entry.examples.is_empty() {
    let _ = writeln!(&mut out, "\n{}", "(no examples)".dimmed());
  } else {
    let _ = writeln!(&mut out, "{} example(s)\n", code_entry.examples.len());

    for (i, example) in code_entry.examples.iter().enumerate() {
      let _ = writeln!(&mut out, "{}", format!("[{i}]:").bold());

      let cirru_str = format_example_node(example);
      for line in cirru_str.lines().filter(|l| !l.trim().is_empty()) {
        let _ = writeln!(&mut out, "  {line}");
      }

      let json = cirru_to_json(example);
      let _ = writeln!(
        &mut out,
        "  {} {}",
        "JSON:".dimmed(),
        serde_json::to_string(&json).unwrap().dimmed()
      );
      let _ = writeln!(&mut out);
    }
  }

  emit_cli_output(&out, render_to_stderr);
  Ok(())
}

fn handle_tests(input_path: &str, namespace: &str, definition: &str) -> Result<(), String> {
  let snapshot = load_snapshot_for_namespace(input_path, namespace)?;
  let file = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;
  if !file.defs.contains_key(definition) && lookup_special_builtin_query_meta(namespace, definition)?.is_some() {
    let mut out = String::new();
    let _ = writeln!(&mut out, "\n{}", "(no tests)".dimmed());
    emit_cli_output(&out, false);
    return Ok(());
  }
  let lookup = resolve_definition_lookup(namespace, definition, file.defs.keys().map(|name| name.as_str()), true)?;
  let render_to_stderr = lookup.warning.is_some();
  if let Some(warning) = lookup.warning.as_deref() {
    print_cli_warning_block(warning);
  }
  let entry = file.defs.get(lookup.resolved.as_str()).expect("resolved definition exists");
  let mut out = String::new();
  if entry.tests.is_empty() {
    let _ = writeln!(&mut out, "\n{}", "(no tests)".dimmed());
  } else {
    let _ = writeln!(&mut out, "{} test(s)\n", entry.tests.len());
    for test in &entry.tests {
      let mut tags = test.tags.iter().map(|tag| format!(":{}", tag.ref_str())).collect::<Vec<_>>();
      tags.sort();
      let _ = writeln!(&mut out, "{}", format!("# {}", test.name).bold());
      if !tags.is_empty() {
        let _ = writeln!(&mut out, "  Tags: {}", tags.join(" "));
      }
      for line in format_example_node(&test.code).lines().filter(|line| !line.trim().is_empty()) {
        let _ = writeln!(&mut out, "  {line}");
      }
      let _ = writeln!(&mut out);
    }
  }
  emit_cli_output(&out, render_to_stderr);
  Ok(())
}

/// Peek definition - show signature/params/doc without full body
fn handle_peek(input_path: &str, namespace: &str, definition: &str) -> Result<(), String> {
  let snapshot = load_snapshot_for_namespace(input_path, namespace)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  if !file_data.defs.contains_key(definition)
    && let Some(meta) = lookup_special_builtin_query_meta(namespace, definition)?
  {
    let mut out = String::new();
    let _ = writeln!(&mut out, "{} {}", "Doc:".bold(), meta.doc);
    let _ = writeln!(&mut out, "{} {}", "Expr:".bold(), meta.expr_preview.dimmed());
    let _ = writeln!(&mut out, "{} {}", "Examples:".bold(), meta.examples.len());
    let _ = writeln!(
      &mut out,
      "{} {}",
      "Schema:".bold(),
      format_query_schema(meta.schema.as_ref(), true).replace('\n', " ").dimmed()
    );
    emit_cli_output(&out, false);
    return Ok(());
  }

  let lookup = resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), true)?;
  let render_to_stderr = lookup.warning.is_some();
  if let Some(warning) = lookup.warning.as_deref() {
    print_cli_warning_block(warning);
  }
  let resolved_definition = lookup.resolved;

  let code_entry = file_data
    .defs
    .get(resolved_definition.as_str())
    .expect("resolved definition exists");

  let mut out = String::new();

  if code_entry.doc.is_empty() {
    let _ = writeln!(&mut out, "{} -", "Doc:".bold());
  } else {
    let _ = writeln!(&mut out, "{} {}", "Doc:".bold(), code_entry.doc);
  }

  match &code_entry.code {
    Cirru::List(items) if !items.is_empty() => {
      let preview = code_entry.code.format_one_liner()?;
      let display = if preview.len() > 120 {
        format!("{}...", &preview[..120])
      } else {
        preview
      };
      let _ = writeln!(&mut out, "{} {}", "Expr:".bold(), display.dimmed());
    }
    Cirru::Leaf(_) => {
      let preview = code_entry.code.format_one_liner()?;
      let _ = writeln!(&mut out, "{} {}", "Leaf:".bold(), preview.dimmed());
    }
    _ => {
      let _ = writeln!(&mut out, "{}", "(empty or invalid definition)".dimmed());
    }
  }

  let _ = writeln!(&mut out, "{} {}", "Examples:".bold(), code_entry.examples.len());
  let _ = writeln!(&mut out, "{} {}", "Tests:".bold(), code_entry.tests.len());

  if let Some(cirru) = query_schema_cirru(code_entry.schema.as_ref(), true)? {
    let preview = format_query_schema_oneline(&cirru)?;
    let display = if preview.len() > 120 {
      format!("{}...", &preview[..120])
    } else {
      preview
    };
    let _ = writeln!(&mut out, "{} {}", "Schema:".bold(), display.dimmed());
  } else {
    let _ = writeln!(&mut out, "{} -", "Schema:".bold());
  }

  emit_cli_output(&out, render_to_stderr);
  Ok(())
}

/// Show definition schema
fn handle_schema(input_path: &str, namespace: &str, definition: &str, json: bool) -> Result<(), String> {
  let snapshot = load_snapshot_for_namespace(input_path, namespace)?;

  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  if !file_data.defs.contains_key(definition)
    && let Some(meta) = lookup_special_builtin_query_meta(namespace, definition)?
  {
    let mut out = String::new();
    if json {
      let id = format!("{namespace}/{definition}");
      let schema_fingerprint = format_query_schema(meta.schema.as_ref(), true);
      println!(
        "{}",
        format_schema_query_json(
          &id,
          "core-builtin",
          meta.schema.as_ref(),
          semantic_revision(&[meta.doc, &schema_fingerprint]),
        )?
      );
      return Ok(());
    } else {
      let _ = writeln!(
        &mut out,
        "{} {}",
        "Schema:".bold(),
        format_query_schema(meta.schema.as_ref(), true).replace('\n', " ").dimmed()
      );
    }
    emit_cli_output(&out, false);
    return Ok(());
  }

  let lookup = resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), true)?;
  let render_to_stderr = lookup.warning.is_some();
  if let Some(warning) = lookup.warning.as_deref() {
    print_cli_warning_block(warning);
  }
  let resolved_definition = lookup.resolved;

  let code_entry = file_data
    .defs
    .get(resolved_definition.as_str())
    .expect("resolved definition exists");

  let mut out = String::new();

  if json {
    let id = format!("{namespace}/{resolved_definition}");
    println!(
      "{}",
      format_schema_query_json(
        &id,
        &namespace_source(&snapshot, namespace),
        code_entry.schema.as_ref(),
        snapshot::definition_revision(code_entry)?,
      )?
    );
    return Ok(());
  }

  if let Some(cirru) = query_schema_cirru(code_entry.schema.as_ref(), true)? {
    let _ = writeln!(&mut out, "{} {}", "Schema:".bold(), format_query_schema_oneline(&cirru)?.dimmed());
  } else {
    let _ = writeln!(&mut out, "{} -", "Schema:".bold());
  }

  emit_cli_output(&out, render_to_stderr);
  Ok(())
}

/// Find symbol across all namespaces
fn handle_find(input_path: &str, symbol: &str, include_deps: bool, detail_offset: usize) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let mut found_definitions: Vec<(String, String)> = vec![];
  let mut found_references: RefResults = vec![]; // (ns, def, context, coords, source)

  for (ns_name, file_data) in &snapshot.files {
    let is_core = ns_name.starts_with("calcit.") || ns_name.starts_with("calcit-test.");

    // Always search for definitions in all namespaces (including core)
    if file_data.defs.contains_key(symbol) {
      found_definitions.push((ns_name.clone(), symbol.to_string()));
    }

    // Search for references only in project namespaces (unless --deps)
    if !include_deps && is_core {
      continue;
    }

    // Search for references in all definitions
    for (def_name, code_entry) in &file_data.defs {
      if find_symbol_in_cirru(&code_entry.code, symbol) {
        let coords = find_symbol_coords(&code_entry.code, symbol);
        found_references.push((
          ns_name.clone(),
          def_name.clone(),
          get_symbol_context_cirru(&code_entry.code, symbol),
          coords,
          "code",
        ));
      }

      if let Ok(Some(schema)) = query_schema_cirru(code_entry.schema.as_ref(), false)
        && find_symbol_in_cirru(&schema, symbol)
      {
        let coords = find_symbol_coords(&schema, symbol);
        found_references.push((
          ns_name.clone(),
          def_name.clone(),
          get_symbol_context_cirru(&schema, symbol),
          coords,
          "schema",
        ));
      }
    }
  }

  // Print summary
  println!(
    "{} {} definition(s), {} reference(s)\n",
    "Matches:".bold(),
    found_definitions.len(),
    found_references.len().saturating_sub(found_definitions.len())
  );

  // Print definitions
  if !found_definitions.is_empty() {
    println!("{}", "Defined in:".bold().green());
    print_detail_window_hint(found_definitions.len(), detail_offset, "definitions");
    for (idx, (ns, def)) in found_definitions.iter().enumerate() {
      if in_detail_window(idx, found_definitions.len(), detail_offset) {
        println!("  {}/{}", ns.cyan(), def.green());
      } else {
        println!("  ⋯ {}/{}", ns.dimmed(), def.dimmed());
      }
    }
    println!();
  }

  // Print references (excluding the definition itself)
  let references: Vec<_> = found_references
    .iter()
    .filter(|(ns, def, _, _, _)| !found_definitions.iter().any(|(dns, ddef)| dns == ns && ddef == def))
    .collect();

  if !references.is_empty() {
    println!("{}", "Referenced in:".bold());
    print_detail_window_hint(references.len(), detail_offset, "references");
    for (idx, (ns, def, context, coords, source)) in references.iter().enumerate() {
      if !in_detail_window(idx, references.len(), detail_offset) {
        println!(
          "  ⋯ {}/{} [{}] ({} path{})",
          ns.dimmed(),
          def.dimmed(),
          source.dimmed(),
          coords.len(),
          if coords.len() == 1 { "" } else { "s" }
        );
        continue;
      }

      // Show main line
      if !context.is_empty() {
        println!("  {}/{} [{}]  {}", ns.cyan(), def, source.dimmed(), context.dimmed());
      } else {
        println!("  {}/{} [{}]", ns.cyan(), def, source.dimmed());
      }

      // Show coordinates on one line with "and" separator
      if !coords.is_empty() {
        let coords_parts: Vec<String> = coords
          .iter()
          .map(|path| {
            let coord_str = format_path(path);
            format!("[{coord_str}]")
          })
          .collect();
        println!("    {}", format!("at {}", coords_parts.join(" and ")).dimmed());
      }
    }
  }

  if found_definitions.is_empty() && references.is_empty() {
    println!("{}", "No matches found.".yellow());
  }

  Ok(())
}

fn collect_usages_from_snapshot(snapshot: &snapshot::Snapshot, target_ns: &str, target_def: &str, include_deps: bool) -> RefResults {
  let mut usages: RefResults = vec![]; // (ns, def, context, coords, source)

  for (ns_name, file_data) in &snapshot.files {
    if !include_deps && (ns_name.starts_with("calcit.") || ns_name.starts_with("calcit-test.")) {
      continue;
    }

    let imports_target = check_ns_imports(&file_data.ns.code, target_ns, target_def);

    for (def_name, code_entry) in &file_data.defs {
      if ns_name == target_ns && def_name == target_def {
        continue;
      }

      let code_symbol = if imports_target || ns_name == target_ns {
        target_def.to_owned()
      } else {
        format!("{target_ns}/{target_def}")
      };

      if find_symbol_in_cirru(&code_entry.code, &code_symbol) {
        let context = get_symbol_context_cirru(&code_entry.code, &code_symbol);
        let coords = find_symbol_coords(&code_entry.code, &code_symbol);
        usages.push((ns_name.clone(), def_name.clone(), context, coords, "code"));
      }

      for test in &code_entry.tests {
        if find_symbol_in_cirru(&test.code, &code_symbol) {
          let expression = get_symbol_context_cirru(&test.code, &code_symbol);
          let context = if expression.is_empty() {
            format!("#{}", test.name)
          } else {
            format!("#{}  {expression}", test.name)
          };
          let coords = find_symbol_coords(&test.code, &code_symbol);
          usages.push((ns_name.clone(), def_name.clone(), context, coords, "tests"));
        }
      }

      if let Ok(Some(schema)) = query_schema_cirru(code_entry.schema.as_ref(), false) {
        let schema_symbol = if imports_target || ns_name == target_ns {
          target_def.to_owned()
        } else {
          format!("{target_ns}/{target_def}")
        };

        if find_symbol_in_cirru(&schema, &schema_symbol) {
          let context = get_symbol_context_cirru(&schema, &schema_symbol);
          let coords = find_symbol_coords(&schema, &schema_symbol);
          usages.push((ns_name.clone(), def_name.clone(), context, coords, "schema"));
        }
      }
    }
  }

  usages.sort_by(|left, right| (&left.0, &left.1, left.4).cmp(&(&right.0, &right.1, right.4)));
  usages
}

/// Find usages of a specific definition
fn handle_usages(input_path: &str, target_ns: &str, target_def: &str, include_deps: bool, detail_offset: usize) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let target_file = snapshot
    .files
    .get(target_ns)
    .ok_or_else(|| format!("Namespace '{target_ns}' not found"))?;
  let lookup = resolve_definition_lookup(target_ns, target_def, target_file.defs.keys().map(|name| name.as_str()), true)?;
  let render_to_stderr = lookup.warning.is_some();
  if let Some(warning) = lookup.warning.as_deref() {
    print_cli_warning_block(warning);
  }
  let resolved_target_def = lookup.resolved;

  let _ = target_file
    .defs
    .get(resolved_target_def.as_str())
    .expect("resolved definition exists");

  let usages = collect_usages_from_snapshot(&snapshot, target_ns, &resolved_target_def, include_deps);

  let mut out = String::new();
  let _ = writeln!(&mut out, "{} {}", "Usages:".bold(), usages.len());

  if usages.is_empty() {
    let _ = writeln!(
      &mut out,
      "\n{}",
      "No usages found. This definition may be unused or only called externally.".yellow()
    );
  } else {
    let _ = writeln!(&mut out);
    if usages.len() > DETAILED_RESULTS_WINDOW {
      let (start, end) = detailed_window(detail_offset, usages.len());
      let _ = writeln!(
        &mut out,
        "{}",
        format!("Detail window for usages: [{start}, {end}) (detail-offset={detail_offset}), other entries are compressed.").dimmed()
      );
    }
    for (idx, (ns, def, context, coords, source)) in usages.iter().enumerate() {
      if !in_detail_window(idx, usages.len(), detail_offset) {
        let _ = writeln!(
          &mut out,
          "  ⋯ {}/{} [{}] ({} path{})",
          ns.dimmed(),
          def.dimmed(),
          source.dimmed(),
          coords.len(),
          if coords.len() == 1 { "" } else { "s" }
        );
        continue;
      }

      if !context.is_empty() {
        let _ = writeln!(
          &mut out,
          "  {}/{} [{}]  {}",
          ns.cyan(),
          def.green(),
          source.dimmed(),
          context.dimmed()
        );
      } else {
        let _ = writeln!(&mut out, "  {}/{} [{}]", ns.cyan(), def.green(), source.dimmed());
      }

      if !coords.is_empty() {
        let coords_parts: Vec<String> = coords
          .iter()
          .map(|path| {
            let coord_str = format_path(path);
            format!("[{coord_str}]")
          })
          .collect();
        let _ = writeln!(&mut out, "    {}", format!("at {}", coords_parts.join(" and ")).dimmed());
      }
    }
  }

  if !usages.is_empty() && command_guidance_enabled() {
    let _ = writeln!(
      &mut out,
      "\n{}",
      "Tip: Modifying this definition may affect the above locations.".dimmed()
    );
  }

  emit_cli_output(&out, render_to_stderr);
  Ok(())
}

// Helper: find all coordinates where symbol appears in Cirru tree
fn find_symbol_coords(code: &Cirru, symbol: &str) -> Vec<Vec<usize>> {
  fn search_recursive(node: &Cirru, symbol: &str, current_path: &[usize], results: &mut Vec<Vec<usize>>) {
    match node {
      Cirru::Leaf(s) if s.as_ref() == symbol => {
        results.push(current_path.to_vec());
      }
      Cirru::List(items) => {
        for (i, item) in items.iter().enumerate() {
          let mut new_path = current_path.to_vec();
          new_path.push(i);
          search_recursive(item, symbol, &new_path, results);
        }
      }
      _ => {}
    }
  }

  let mut results = Vec::new();
  search_recursive(code, symbol, &[], &mut results);
  results
}

// Helper: recursively search for symbol in Cirru tree
fn find_symbol_in_cirru(code: &Cirru, symbol: &str) -> bool {
  match code {
    Cirru::Leaf(s) => s.as_ref() == symbol,
    Cirru::List(items) => items.iter().any(|item| find_symbol_in_cirru(item, symbol)),
  }
}

// Helper: get context around symbol usage in Cirru format (compact)
// Returns the smallest expression containing the symbol
fn get_symbol_context_cirru(code: &Cirru, symbol: &str) -> String {
  fn find_smallest_containing(node: &Cirru, symbol: &str) -> Option<Cirru> {
    match node {
      Cirru::Leaf(s) if s.as_ref() == symbol => Some(node.clone()),
      Cirru::List(items) => {
        for item in items {
          if let Some(found) = find_smallest_containing(item, symbol) {
            if matches!(found, Cirru::Leaf(_)) {
              return Some(node.clone());
            }
            return Some(found);
          }
        }
        None
      }
      _ => None,
    }
  }

  if let Some(context_node) = find_smallest_containing(code, symbol) {
    let cirru_str = context_node.format_one_liner().unwrap_or_default();
    let trimmed = cirru_str.trim();
    if trimmed.len() > 50 {
      return format!("{}...", &trimmed[..50]);
    }
    return trimmed.to_string();
  }
  String::new()
}

// Helper: check if namespace imports the target
fn check_ns_imports(ns_code: &Cirru, target_ns: &str, _target_def: &str) -> bool {
  // ns_code is like (ns my-ns (:require [target-ns ...]))
  // Simplified check: just see if target_ns appears in the ns declaration
  match ns_code {
    Cirru::Leaf(s) => s.as_ref() == target_ns,
    Cirru::List(items) => items.iter().any(|item| check_ns_imports(item, target_ns, _target_def)),
  }
}

/// Fuzzy search for namespace/definition by pattern
/// Searches for `<pattern>` in qualified names like `namespace/definition`
fn handle_fuzzy_search(input_path: &str, pattern: &str, include_deps: bool, limit: usize, detail_offset: usize) -> Result<(), String> {
  let snapshot = load_snapshot(input_path)?;

  let pattern_lower = pattern.to_lowercase();
  let mut results: Vec<(String, String, bool)> = Vec::new(); // (namespace, def, is_core)

  // Search in all files
  for (ns_name, file_data) in &snapshot.files {
    // Mark core namespaces as "dep" for display
    let is_core = ns_name.starts_with("calcit.") || ns_name.starts_with("calcit-test.");

    // Skip core namespaces unless deps is requested
    if !include_deps && is_core {
      continue;
    }

    for def_name in file_data.defs.keys() {
      let qualified = format!("{ns_name}/{def_name}");
      let qualified_lower = qualified.to_lowercase();

      // Fuzzy match: check if pattern appears in qualified name
      if fuzzy_match(&qualified_lower, &pattern_lower) {
        results.push((ns_name.clone(), def_name.clone(), is_core));
      }
    }
  }

  // Sort results by relevance (exact prefix match first, then alphabetically)
  results.sort_by(|(ns_a, def_a, _), (ns_b, def_b, _)| {
    let qualified_a = format!("{ns_a}/{def_a}").to_lowercase();
    let qualified_b = format!("{ns_b}/{def_b}").to_lowercase();

    // Prioritize exact prefix matches
    let a_prefix = qualified_a.starts_with(&pattern_lower);
    let b_prefix = qualified_b.starts_with(&pattern_lower);

    match (a_prefix, b_prefix) {
      (true, false) => std::cmp::Ordering::Less,
      (false, true) => std::cmp::Ordering::Greater,
      _ => qualified_a.cmp(&qualified_b),
    }
  });

  // Limit results
  let total = results.len();
  let displayed: Vec<_> = results.into_iter().take(limit).collect();

  println!("{} {} results", "Search:".bold(), total);

  if displayed.is_empty() {
    println!("  {}", "No matches found".dimmed());
    if command_guidance_enabled() {
      println!(
        "\n{}",
        "Tip: Try a broader pattern, or add --deps to include core namespaces.".dimmed()
      );
    }
    return Ok(());
  }

  print_detail_window_hint(displayed.len(), detail_offset, "search results");

  for (idx, (ns, def, is_core)) in displayed.iter().enumerate() {
    if !in_detail_window(idx, displayed.len(), detail_offset) {
      println!("  ⋯ {}/{}", ns.dimmed(), def.dimmed());
      continue;
    }

    let qualified = format!("{}/{}", ns.cyan(), def.green());
    if *is_core {
      println!("  {} {}", qualified, "(core)".dimmed());
    } else {
      println!("  {qualified}");
    }
  }

  if total > limit {
    println!("  ⋯ {} more results...", total - limit);
  }

  if command_guidance_enabled() {
    println!("\n{}", "Tip: Use `query def <ns/def>` to view definition content.".dimmed());
  }

  Ok(())
}

/// Simple fuzzy matching: check if all characters of pattern appear in order in text
fn fuzzy_match(text: &str, pattern: &str) -> bool {
  // Support multiple match styles:
  // 1. Substring match: "map" matches "hash-map"
  // 2. Character sequence match: "hm" matches "hash-map"

  // First try substring match (fast path)
  if text.contains(pattern) {
    return true;
  }

  // Then try character sequence match
  let mut text_chars = text.chars().peekable();
  for pattern_char in pattern.chars() {
    loop {
      match text_chars.next() {
        Some(c) if c == pattern_char => break,
        Some(_) => continue,
        None => return false,
      }
    }
  }
  true
}

fn snapshot_code_path(path: &[usize]) -> String {
  if path.is_empty() {
    "code".to_owned()
  } else {
    format!("code{}", format_path(path))
  }
}

fn format_search_results_json(
  command: &'static str,
  pattern: &str,
  pattern_is_json: bool,
  start_path: Option<&str>,
  common_opts: &SearchCommonOpts,
  snapshot: &snapshot::Snapshot,
  results: &SearchResults,
) -> Result<String, String> {
  let total_matches = results.iter().map(|(_, _, matches)| matches.len()).sum::<usize>();
  let mut cursor_index = 0_usize;
  let definitions = results
    .iter()
    .map(|(namespace, definition, matches)| {
      serde_json::json!({
        "id": format!("{namespace}/{definition}"),
        "namespace": namespace,
        "name": definition,
        "match_count": matches.len(),
        "matches": matches.iter().map(|(path, node)| {
          let current_cursor_index = cursor_index;
          cursor_index += 1;
          let parent_path = common_opts.parent_path.then(|| {
            if path.is_empty() {
              None
            } else {
              Some(snapshot_code_path(&path[..path.len() - 1]))
            }
          }).flatten();
          serde_json::json!({
            "cursor_index": current_cursor_index,
            "path": snapshot_code_path(path),
            "parent_path": parent_path,
            "tree": cirru_to_json_value(node),
          })
        }).collect::<Vec<_>>(),
      })
    })
    .collect::<Vec<_>>();

  let mut revision_parts = vec![
    command.to_owned(),
    pattern.to_owned(),
    common_opts.filter.unwrap_or_default().to_owned(),
    start_path.unwrap_or_default().to_owned(),
  ];
  for (namespace, definition, _) in results {
    let entry = snapshot
      .files
      .get(namespace)
      .and_then(|file| file.defs.get(definition))
      .ok_or_else(|| format!("Definition disappeared while formatting search results: {namespace}/{definition}"))?;
    revision_parts.push(format!("{namespace}/{definition}:{}", snapshot::definition_revision(entry)?));
  }
  let revision_refs = revision_parts.iter().map(String::as_str).collect::<Vec<_>>();

  let envelope = serde_json::json!({
    "schema_version": 1,
    "command": command,
    "revision": semantic_revision(&revision_refs),
    "data": {
      "pattern": pattern,
      "filters": {
        "definition": common_opts.filter,
        "exact": !common_opts.loose,
        "regex": common_opts.regex,
        "max_depth": common_opts.max_depth,
        "start_path": start_path,
        "entry": common_opts.entry,
        "pattern_is_json": pattern_is_json,
      },
      "summary": {
        "definitions": results.len(),
        "matches": total_matches,
      },
      "definitions": definitions,
    },
    "diagnostics": [],
  });
  serde_json::to_string_pretty(&envelope).map_err(|error| format!("Failed to encode search JSON: {error}"))
}

fn maybe_set_cursor_from_search_results(
  input_path: &str,
  results: &SearchResults,
  selected_index: usize,
  last_query: CursorLastQuery,
) -> Result<(), String> {
  let total_matches = results.iter().map(|(_, _, matches)| matches.len()).sum::<usize>();
  let main_snapshot = load_main_snapshot(input_path)?;
  let mut current_index = 0_usize;
  for (namespace, definition, matches) in results {
    for (path, _) in matches {
      if current_index == selected_index {
        if !main_snapshot.files.contains_key(namespace) {
          return Err(format!(
            "Search match #{selected_index} belongs to dependency or builtin namespace '{namespace}', which cannot become a project edit cursor. Narrow the search with `--filter <project-namespace>` and choose its displayed cursor index."
          ));
        }
        return set_cursor_from_query_match(
          input_path,
          &format!("{namespace}/{definition}"),
          path.clone(),
          selected_index,
          last_query,
        );
      }
      current_index += 1;
    }
  }
  Err(format!(
    "Search cursor index {selected_index} is out of range; query returned {total_matches} match(es)."
  ))
}

fn snapshot_content_revision(input_path: &str) -> Result<String, String> {
  let content = fs::read(input_path).map_err(|error| format!("Failed to read snapshot revision from '{input_path}': {error}"))?;
  let mut hasher = Md5::new();
  hasher.update(&content);
  Ok(format!("md5:{}", hex::encode(hasher.finalize())))
}

fn cursor_last_query(
  input_path: &str,
  command: &str,
  pattern: &str,
  pattern_is_json: bool,
  start_path: Option<&str>,
  common_opts: &SearchCommonOpts,
  selected_index: usize,
) -> Result<CursorLastQuery, String> {
  Ok(CursorLastQuery {
    command: command.to_string(),
    pattern: pattern.to_string(),
    filter: common_opts.filter.map(str::to_string),
    exact: !common_opts.loose,
    regex: common_opts.regex,
    max_depth: common_opts.max_depth,
    start_path: start_path.map(str::to_string),
    entry: common_opts.entry.map(str::to_string),
    pattern_is_json,
    selected_index,
    snapshot_revision: snapshot_content_revision(input_path)?,
  })
}

fn handle_repeat_cursor_search(input_path: &str, forward: bool) -> Result<(), String> {
  let last_query = load_cursor_last_query(input_path)?;
  let selected_index = if forward {
    last_query
      .selected_index
      .checked_add(1)
      .ok_or("Saved query result index overflowed.")?
  } else {
    last_query
      .selected_index
      .checked_sub(1)
      .ok_or("The saved query cursor is already at the first result.")?
  };
  let current_revision = snapshot_content_revision(input_path)?;
  if current_revision != last_query.snapshot_revision {
    return Err(
      "Snapshot changed since the saved cursor search. Refusing to reuse its result index; rerun the original search with `--set-cursor <index>`."
        .to_string(),
    );
  }
  let common_opts = SearchCommonOpts {
    filter: last_query.filter.as_deref(),
    loose: !last_query.exact,
    regex: last_query.regex,
    max_depth: last_query.max_depth,
    entry: last_query.entry.as_deref(),
    detail_offset: 0,
    parent_path: false,
    format: QueryRenderFormat::Human,
    set_cursor: Some(selected_index),
    compact_output: true,
  };
  match last_query.command.as_str() {
    "search" => handle_search_leaf(input_path, &last_query.pattern, last_query.start_path.as_deref(), &common_opts),
    "search-expr" => handle_search_expr(
      input_path,
      &last_query.pattern,
      last_query.pattern_is_json,
      last_query.start_path.as_deref(),
      &common_opts,
    ),
    other => Err(format!("Saved cursor query command '{other}' is not repeatable.")),
  }
}

fn parse_search_start_path(start_path: Option<&str>) -> Result<Option<Vec<usize>>, String> {
  start_path
    .map(|path| parse_path(path).map_err(|error| format!("Invalid start path '{path}': {error}")))
    .transpose()
}

fn parse_search_filter(filter: Option<&str>) -> Result<(Option<&str>, Option<&str>), String> {
  let Some(filter) = filter else {
    return Ok((None, None));
  };
  let mut parts = filter.split('/');
  let namespace = parts.next();
  let definition = parts.next();
  if parts.next().is_some() {
    return Err(format!(
      "Invalid filter format: '{filter}'. Use 'namespace' or 'namespace/definition'"
    ));
  }
  Ok((namespace, definition))
}

fn collect_search_results<F>(
  snapshot: &snapshot::Snapshot,
  start_path: Option<&[usize]>,
  filter: Option<&str>,
  mut search: F,
) -> Result<SearchResults, String>
where
  F: FnMut(&Cirru, &[usize]) -> Vec<(Vec<usize>, Cirru)>,
{
  let (filter_ns, filter_def) = parse_search_filter(filter)?;
  let mut all_results: SearchResults = Vec::new();

  for (ns, file_data) in &snapshot.files {
    if filter_ns.is_some_and(|filter_namespace| ns != filter_namespace) {
      continue;
    }
    for (def_name, code_entry) in &file_data.defs {
      if filter_def.is_some_and(|filter_definition| def_name != filter_definition) {
        continue;
      }
      let owned_search_root = if let Some(path) = start_path {
        match navigate_to_path(&code_entry.code, path) {
          Ok(node) => Some(node),
          Err(error) => {
            eprintln!(
              "{} Failed to navigate to start path in {}/{}: {}",
              "Warning:".yellow(),
              ns,
              def_name,
              error
            );
            continue;
          }
        }
      } else {
        None
      };
      let search_root = owned_search_root.as_ref().unwrap_or(&code_entry.code);
      let results = search(search_root, start_path.unwrap_or(&[]));
      if !results.is_empty() {
        all_results.push((ns.clone(), def_name.clone(), results));
      }
    }
  }

  all_results.sort_by(|a, b| b.2.len().cmp(&a.2.len()).then_with(|| a.0.cmp(&b.0)).then_with(|| a.1.cmp(&b.1)));
  Ok(all_results)
}

#[derive(Clone, Copy)]
struct SearchResultDisplay<'a> {
  highlight_target: Option<&'a str>,
  bracket_path: bool,
  show_parent_path: bool,
}

fn print_search_results_human(
  snapshot: &snapshot::Snapshot,
  all_results: &SearchResults,
  common_opts: &SearchCommonOpts,
  display: SearchResultDisplay<'_>,
) {
  if all_results.is_empty() {
    println!("{}", "No matches found.".yellow());
    return;
  }

  let total_matches: usize = all_results.iter().map(|(_, _, results)| results.len()).sum();
  println!(
    "{} {} match(es) found in {} definition(s):\n",
    "Results:".bold().green(),
    total_matches,
    all_results.len()
  );

  let mut definition_offset = 0_usize;
  for (ns, def_name, results) in all_results {
    println!("{} {}/{} ({} matches)", "●".cyan(), ns.dimmed(), def_name.green(), results.len());
    print_detail_window_hint(results.len(), common_opts.detail_offset, "matches");

    if let Some(file_data) = snapshot.files.get(ns)
      && let Some(code_entry) = file_data.defs.get(def_name)
    {
      let total = results.len();
      let (start, end) = detailed_window(common_opts.detail_offset, total);
      let detailed_count = end.saturating_sub(start);

      for (local_index, (path, node)) in results.iter().enumerate().skip(start).take(detailed_count) {
        let cursor_index = definition_offset + local_index;
        if path.is_empty() {
          let (content, truncated) = preview_node_oneline(&code_entry.code, 110);
          if truncated {
            println!(
              "    {} {} {} ⟪…⟫",
              format!("[#{cursor_index}]").cyan(),
              "(root)".cyan(),
              content.dimmed()
            );
          } else {
            println!(
              "    {} {} {}",
              format!("[#{cursor_index}]").cyan(),
              "(root)".cyan(),
              content.dimmed()
            );
          }
        } else {
          let path_str = format_path(path);
          let path_label = if display.bracket_path { format!("[{path_str}]") } else { path_str };
          let ((expr_preview, expr_truncated), parent_previews) =
            expression_and_parent_preview(&code_entry.code, path, node, display.highlight_target, common_opts.loose);
          let (display_preview, display_truncated) = parent_previews
            .first()
            .map(|(text, truncated)| (text.as_str(), *truncated))
            .unwrap_or((expr_preview.as_str(), expr_truncated));
          if display_truncated {
            println!(
              "    {} {} {} ⟪…⟫",
              format!("[#{cursor_index}]").cyan(),
              path_label.cyan(),
              display_preview
            );
          } else {
            println!(
              "    {} {} {}",
              format!("[#{cursor_index}]").cyan(),
              path_label.cyan(),
              display_preview
            );
          }
          if display.show_parent_path && common_opts.parent_path {
            let parent_path_str = snapshot_code_path(&path[..path.len() - 1]);
            println!("       {} {}", "parent:".dimmed(), parent_path_str.dimmed());
          }
        }
      }

      if start > 0 {
        println!(
          "    {}",
          format!(
            "[#{}..#{}] {start} matches compressed before window",
            definition_offset,
            definition_offset + start - 1
          )
          .dimmed()
        );
      }
      if end < total {
        println!(
          "    {}",
          format!(
            "[#{}..#{}] {} matches compressed after window",
            definition_offset + end,
            definition_offset + total - 1,
            total - end
          )
          .dimmed()
        );
      }
    }

    definition_offset += results.len();
    println!();
  }

  let mut tips = Tips::new();
  if total_matches > 10 && common_opts.loose {
    tips.add_with_priority(
      TipPriority::High,
      format!(
        "Many matches ({total_matches}); add {} to show exact matches only",
        "--exact".yellow()
      ),
    );
  }
  tips.print();
}

struct SearchCommandInfo<'a> {
  envelope_command: &'static str,
  cursor_command: &'static str,
  pattern: &'a str,
  pattern_is_json: bool,
  start_path: Option<&'a str>,
  display: SearchResultDisplay<'a>,
}

fn finish_search_results(
  input_path: &str,
  snapshot: &snapshot::Snapshot,
  all_results: &SearchResults,
  common_opts: &SearchCommonOpts,
  info: SearchCommandInfo<'_>,
) -> Result<(), String> {
  if let Some(selected_index) = common_opts.set_cursor {
    let last_query = cursor_last_query(
      input_path,
      info.cursor_command,
      info.pattern,
      info.pattern_is_json,
      info.start_path,
      common_opts,
      selected_index,
    )?;
    maybe_set_cursor_from_search_results(input_path, all_results, selected_index, last_query)?;
  }

  if common_opts.compact_output {
    return Ok(());
  }
  if common_opts.format == QueryRenderFormat::Json {
    println!(
      "{}",
      format_search_results_json(
        info.envelope_command,
        info.pattern,
        info.pattern_is_json,
        info.start_path,
        common_opts,
        snapshot,
        all_results,
      )?
    );
    return Ok(());
  }
  print_search_results_human(snapshot, all_results, common_opts, info.display);
  Ok(())
}

/// Search for leaf nodes (strings) in a definition
fn handle_search_leaf(input_path: &str, pattern: &str, start_path: Option<&str>, common_opts: &SearchCommonOpts) -> Result<(), String> {
  let snapshot = load_snapshot_for_search(input_path, common_opts)?;
  let parsed_start_path = parse_search_start_path(start_path)?;
  let all_results = collect_search_results(
    &snapshot,
    parsed_start_path.as_deref(),
    common_opts.filter,
    |search_root, base_path| {
      search_leaf_nodes(
        search_root,
        pattern,
        common_opts.loose,
        common_opts.regex,
        common_opts.max_depth,
        base_path,
      )
    },
  )?;

  finish_search_results(
    input_path,
    &snapshot,
    &all_results,
    common_opts,
    SearchCommandInfo {
      envelope_command: "query.search",
      cursor_command: "search",
      pattern,
      pattern_is_json: false,
      start_path,
      display: SearchResultDisplay {
        highlight_target: Some(pattern),
        bracket_path: false,
        show_parent_path: true,
      },
    },
  )
}

/// Search for structural expressions across project or in filtered scope
fn handle_search_expr(
  input_path: &str,
  pattern: &str,
  json: bool,
  start_path: Option<&str>,
  common_opts: &SearchCommonOpts,
) -> Result<(), String> {
  let snapshot = load_snapshot_for_search(input_path, common_opts)?;
  let parsed_start_path = parse_search_start_path(start_path)?;

  let pattern_node = if json {
    let json_val: serde_json::Value = serde_json::from_str(pattern).map_err(|e| format!("Failed to parse JSON pattern: {e}"))?;
    json_to_cirru(&json_val)?
  } else {
    cirru_parser::parse(pattern)
      .map_err(|e| format!("Failed to parse Cirru pattern: {e}"))?
      .first()
      .ok_or("Pattern is empty")?
      .clone()
  };

  let highlight_target: Option<&str> = match &pattern_node {
    Cirru::Leaf(s) => Some(s.as_ref()),
    _ => None,
  };

  let all_results = collect_search_results(
    &snapshot,
    parsed_start_path.as_deref(),
    common_opts.filter,
    |search_root, base_path| search_expr_nodes(search_root, &pattern_node, common_opts.loose, common_opts.max_depth, base_path),
  )?;

  finish_search_results(
    input_path,
    &snapshot,
    &all_results,
    common_opts,
    SearchCommandInfo {
      envelope_command: "query.search-expr",
      cursor_command: "search-expr",
      pattern,
      pattern_is_json: json,
      start_path,
      display: SearchResultDisplay {
        highlight_target,
        bracket_path: true,
        show_parent_path: false,
      },
    },
  )
}

/// Helper function to convert JSON to Cirru
fn json_to_cirru(json: &serde_json::Value) -> Result<Cirru, String> {
  match json {
    serde_json::Value::String(s) => Ok(Cirru::Leaf(s.as_str().into())),
    serde_json::Value::Array(arr) => {
      let items: Result<Vec<_>, _> = arr.iter().map(json_to_cirru).collect();
      Ok(Cirru::List(items?))
    }
    _ => Err("Pattern must be a string or array".to_string()),
  }
}

/// Print search results with parent context
/// Search for leaf nodes with exact or loose matching
fn search_leaf_nodes(
  node: &Cirru,
  pattern: &str,
  loose: bool,
  regex: bool,
  max_depth: usize,
  current_path: &[usize],
) -> Vec<(Vec<usize>, Cirru)> {
  let mut results = Vec::new();

  // Check depth limit
  if max_depth > 0 && current_path.len() >= max_depth {
    return results;
  }

  // Compile regex once if needed
  let regex_pattern = if regex {
    match regex::Regex::new(pattern) {
      Ok(r) => Some(r),
      Err(e) => {
        eprintln!("{} Invalid regex '{}': {}", "Error:".red().bold(), pattern, e);
        return results;
      }
    }
  } else {
    None
  };

  // Only match leaf nodes
  match node {
    Cirru::Leaf(s) => {
      let matches = if regex {
        regex_pattern.as_ref().is_some_and(|r| r.is_match(s))
      } else if loose {
        // Loose: check if leaf contains pattern
        s.to_lowercase().contains(&pattern.to_lowercase())
      } else {
        // Exact: check if leaf equals pattern
        s.as_ref() == pattern
      };

      if matches {
        results.push((current_path.to_vec(), node.clone()));
      }
    }
    Cirru::List(items) => {
      // Recursively search children
      for (i, item) in items.iter().enumerate() {
        let mut new_path = current_path.to_vec();
        new_path.push(i);
        results.extend(search_leaf_nodes(item, pattern, loose, regex, max_depth, &new_path));
      }
    }
  }

  results
}

fn search_expr_nodes(node: &Cirru, pattern: &Cirru, loose: bool, max_depth: usize, current_path: &[usize]) -> Vec<(Vec<usize>, Cirru)> {
  let mut results = Vec::new();

  if max_depth > 0 && current_path.len() >= max_depth {
    return results;
  }

  // Check if current node matches pattern
  let matches = if loose {
    contains_pattern(node, pattern)
  } else {
    matches_exact_structure(node, pattern)
  };

  if matches {
    results.push((current_path.to_vec(), node.clone()));
  }

  // Recursively search children
  if let Cirru::List(items) = node {
    for (i, item) in items.iter().enumerate() {
      let mut new_path = current_path.to_vec();
      new_path.push(i);
      results.extend(search_expr_nodes(item, pattern, loose, max_depth, &new_path));
    }
  }

  results
}

/// Check if node starts with pattern (prefix matching)
/// In loose mode, pattern must match from the beginning of the list
fn contains_pattern(node: &Cirru, pattern: &Cirru) -> bool {
  match (node, pattern) {
    // Leaf nodes: check string containment
    (Cirru::Leaf(s), Cirru::Leaf(p)) => s.to_lowercase().contains(&p.as_ref().to_lowercase()),

    // List: pattern must match from the beginning (prefix match)
    (Cirru::List(items), Cirru::List(pattern_items)) => {
      if pattern_items.is_empty() {
        return true;
      }

      // Pattern must not be longer than the actual list
      if pattern_items.len() > items.len() {
        return false;
      }

      // Check if pattern matches from the beginning using prefix matching
      for (i, pattern_item) in pattern_items.iter().enumerate() {
        if !matches_prefix_structure(&items[i], pattern_item) {
          return false;
        }
      }
      true
    }

    _ => false,
  }
}

/// Check if node matches pattern as a prefix (allows node to be longer than pattern)
fn matches_prefix_structure(node: &Cirru, pattern: &Cirru) -> bool {
  match (node, pattern) {
    (Cirru::Leaf(s1), Cirru::Leaf(s2)) => s1.as_ref() == s2.as_ref(),
    (Cirru::List(items1), Cirru::List(items2)) => {
      // Pattern must not be longer than node
      if items2.len() > items1.len() {
        return false;
      }
      // Check if pattern matches the prefix of node
      items2
        .iter()
        .enumerate()
        .all(|(i, pattern_item)| matches_prefix_structure(&items1[i], pattern_item))
    }
    _ => false,
  }
}

/// Check if node exactly matches pattern structure
fn matches_exact_structure(node: &Cirru, pattern: &Cirru) -> bool {
  match (node, pattern) {
    (Cirru::Leaf(s1), Cirru::Leaf(s2)) => s1.as_ref() == s2.as_ref(),
    (Cirru::List(items1), Cirru::List(items2)) => {
      items1.len() == items2.len() && items1.iter().zip(items2.iter()).all(|(n1, n2)| matches_exact_structure(n1, n2))
    }
    _ => false,
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// L4: path expression resolver
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_query_path(input_path: &str, opts: &QueryPathCommand) -> Result<(), String> {
  let snapshot = load_snapshot_for_namespace(input_path, &opts.namespace)?;
  let file_data = snapshot
    .files
    .get(opts.namespace.as_str())
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  // Collect all definitions as the top-level nodes to search within
  // For each def, we'll search the def's code
  // Simple approach: just search within each def's code
  for (def_name, code_entry) in &file_data.defs {
    match resolve_path_expression(&code_entry.code, &opts.selector) {
      Ok(path) => {
        println!("{} {} -> {}", opts.namespace, def_name, format_path(&path));
        return Ok(());
      }
      Err(_) => continue,
    }
  }
  Err(format!("Path expression not found in namespace '{}'", opts.namespace))
}

pub(crate) fn resolve_path_expression(root: &Cirru, selector: &str) -> Result<Vec<usize>, String> {
  // Parse the selector as Cirru
  let parsed = cirru_parser::parse(selector).map_err(|e| format!("Failed to parse path selector: {e}"))?;
  let Some(first) = parsed.first() else {
    return Err("Empty path selector".to_string());
  };

  // Verify it starts with "path"
  let Cirru::List(steps) = first else {
    return Err("Path expression must be a list starting with 'path'".to_string());
  };
  if steps.is_empty() {
    return Err("Empty path expression".to_string());
  }
  let Cirru::Leaf(head) = &steps[0] else {
    return Err("Path expression must start with 'path'".to_string());
  };
  if head.as_ref() != "path" {
    return Err(format!("Expected 'path' at start, got '{head}'"));
  }

  // Walk the selectors
  let mut current_node = root.clone();
  let mut current_path: Vec<usize> = vec![];

  for step in &steps[1..] {
    match step {
      Cirru::Leaf(leaf_val) => {
        // Bare leaf: match the current node as a leaf
        let Cirru::Leaf(current_leaf) = &current_node else {
          return Err(format!("Expected leaf but got list at path {}", format_path(&current_path)));
        };
        if current_leaf.as_ref() != leaf_val.as_ref() {
          return Err(format!(
            "Leaf mismatch at path {}: expected {:?}, got {:?}",
            format_path(&current_path),
            leaf_val.as_ref(),
            current_leaf.as_ref()
          ));
        }
        // Leaf matched — search next sibling by going up and right
        // For simplicity, we just verify and move on
      }
      Cirru::List(selector_list) => {
        if selector_list.is_empty() {
          return Err("Empty selector".to_string());
        }
        let Cirru::Leaf(op) = &selector_list[0] else {
          return Err("Selector must start with an operator leaf".to_string());
        };
        match op.as_ref() {
          "heading" => {
            // Match current node's children against pattern
            let Cirru::List(ref current_children) = current_node else {
              return Err("heading requires a list node".to_string());
            };
            let pattern = &selector_list[1..];
            if !starts_with_pattern(current_children, pattern) {
              return Err(format!(
                "heading mismatch at path {}: node does not start with expected pattern",
                format_path(&current_path)
              ));
            }
          }
          "nth" => {
            if selector_list.len() < 2 {
              return Err("nth requires an index argument".to_string());
            }
            let Cirru::Leaf(idx_leaf) = &selector_list[1] else {
              return Err("nth index must be a number".to_string());
            };
            let idx: usize = idx_leaf
              .as_ref()
              .parse()
              .map_err(|_| format!("Invalid nth index: '{}'", idx_leaf.as_ref()))?;
            let Cirru::List(ref children) = current_node else {
              return Err("nth requires a list node".to_string());
            };
            if idx >= children.len() {
              return Err(format!("nth index {} out of bounds ({} children)", idx, children.len()));
            }
            current_path.push(idx);
            current_node = children[idx].clone();
          }
          _ => return Err(format!("Unknown selector: '{}'", op.as_ref())),
        }
      }
    }
  }

  Ok(current_path)
}

fn starts_with_pattern(node_children: &[Cirru], pattern: &[Cirru]) -> bool {
  if pattern.len() > node_children.len() {
    return false;
  }
  for (i, pat) in pattern.iter().enumerate() {
    match pat {
      Cirru::Leaf(pat_leaf) => {
        let Cirru::Leaf(child_leaf) = &node_children[i] else {
          return false;
        };
        if child_leaf.as_ref() != pat_leaf.as_ref() {
          return false;
        }
      }
      Cirru::List(pat_list) => {
        let Cirru::List(child_list) = &node_children[i] else {
          return false;
        };
        if !starts_with_pattern(child_list, pat_list) {
          return false;
        }
      }
    }
  }
  true
}

// ═══════════════════════════════════════════════════════════════════════════════
// Phase 4: anchor annotations
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_query_anchors(input_path: &str, opts: &QueryAnchorsCommand) -> Result<(), String> {
  let snapshot = load_snapshot_for_namespace(input_path, &opts.namespace)?;
  let file_data = snapshot
    .files
    .get(opts.namespace.as_str())
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  for (def_name, code_entry) in &file_data.defs {
    let anchors = find_anchors(&code_entry.code, &[]);
    for (path, anchor_name) in anchors {
      println!("  @anchor:{anchor_name} -> {}/{def_name} {}", opts.namespace, format_path(&path));
    }
  }
  Ok(())
}

fn find_anchors(node: &Cirru, current_path: &[usize]) -> Vec<(Vec<usize>, String)> {
  let mut results = vec![];
  if let Cirru::List(children) = node {
    // Look for `noted @anchor:<name> expr` pattern
    if children.len() >= 3
      && let Cirru::Leaf(first) = &children[0]
      && first.as_ref() == "noted"
      && let Cirru::Leaf(tag) = &children[1]
    {
      let tag_str = tag.as_ref();
      if let Some(name) = tag_str.strip_prefix("@anchor:") {
        results.push((current_path.to_vec(), name.to_string()));
      }
    }
    // Recurse into children
    for (i, child) in children.iter().enumerate() {
      let mut child_path = current_path.to_vec();
      child_path.push(i);
      results.extend(find_anchors(child, &child_path));
    }
  }
  results
}
