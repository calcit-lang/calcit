//! Common utilities shared between CLI handlers

use cirru_edn::Edn;
use cirru_parser::Cirru;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::cli_args::SyntaxInputFormat;

// Error message constants
pub const ERR_MULTIPLE_INPUT_SOURCES: &str = "Multiple input sources provided. Use only one of: --file, --code, or stdin.";

pub const ERR_CODE_INPUT_REQUIRED: &str =
  "Code input required: use --file, --code (with a `quote` code/data boundary), or pipe/redirect input via stdin";

pub const ERR_JSON_OBJECTS_NOT_SUPPORTED: &str = "JSON objects not supported, use arrays";

/// Convert JSON Value to Cirru syntax tree
pub fn json_value_to_cirru(json: &serde_json::Value) -> Result<Cirru, String> {
  match json {
    serde_json::Value::String(s) => Ok(Cirru::Leaf(Arc::from(s.as_str()))),
    serde_json::Value::Number(n) => Ok(Cirru::Leaf(Arc::from(n.to_string()))),
    serde_json::Value::Bool(b) => Ok(Cirru::Leaf(Arc::from(b.to_string()))),
    serde_json::Value::Null => Ok(Cirru::Leaf(Arc::from("nil"))),
    serde_json::Value::Array(arr) => {
      let items: Result<Vec<Cirru>, String> = arr.iter().map(json_value_to_cirru).collect();
      Ok(Cirru::List(items?))
    }
    serde_json::Value::Object(_) => Err(ERR_JSON_OBJECTS_NOT_SUPPORTED.to_string()),
  }
}

/// Convert Cirru syntax tree to JSON value (internal)
pub fn cirru_to_json_value(c: &Cirru) -> serde_json::Value {
  match c {
    Cirru::Leaf(s) => serde_json::Value::String(s.to_string()),
    Cirru::List(items) => serde_json::Value::Array(items.iter().map(cirru_to_json_value).collect()),
  }
}

/// Convert Cirru syntax tree to JSON string
pub fn cirru_to_json(node: &Cirru) -> String {
  serde_json::to_string_pretty(&cirru_to_json_value(node)).unwrap_or_else(|_| "[]".to_string())
}

/// Render an explicit Markdown boundary around code or structured text.
pub fn markdown_fenced_block(language: &str, content: &str) -> String {
  let longest_backtick_run = content.split(|character| character != '`').map(str::len).max().unwrap_or_default();
  let fence = "`".repeat(longest_backtick_run.saturating_add(1).max(3));
  let mut out = String::new();
  let _ = writeln!(&mut out, "{fence}{language}");
  out.push_str(content.trim_matches('\n'));
  out.push('\n');
  let _ = writeln!(&mut out, "{fence}");
  out
}

pub fn format_cirru_source(node: &Cirru) -> Result<String, String> {
  match node {
    Cirru::Leaf(value) => Ok(cirru_parser::generate_leaf(value)),
    Cirru::List(_) => cirru_parser::format(std::slice::from_ref(node), cirru_parser::CirruWriterOptions { use_inline: false })
      .map(|content| content.trim().to_owned())
      .map_err(|error| format!("Failed to format Cirru source preview: {error}")),
  }
}

pub fn markdown_cirru_section(level: usize, title: &str, node: &Cirru, max_lines: usize) -> Result<String, String> {
  let source = format_cirru_source(node)?;
  let lines = source.lines().collect::<Vec<_>>();
  let truncated = max_lines > 0 && lines.len() > max_lines;
  let visible = if truncated { lines[..max_lines].join("\n") } else { source.clone() };
  let mut out = String::new();
  let _ = writeln!(&mut out, "{} {title}\n", "#".repeat(level.max(1)));
  let _ = writeln!(&mut out, "- node kind: `{}`\n", syntax_node_kind(node));
  out.push_str(&markdown_fenced_block("cirru", &visible));
  if truncated {
    let _ = writeln!(&mut out, "\n_Preview truncated: showing {max_lines} of {} lines._", lines.len());
  }
  Ok(out)
}

pub fn markdown_json_section(level: usize, title: &str, value: &serde_json::Value) -> Result<String, String> {
  let content = serde_json::to_string_pretty(value).map_err(|error| format!("Failed to format JSON preview: {error}"))?;
  Ok(format!(
    "{} {title}\n\n{}",
    "#".repeat(level.max(1)),
    markdown_fenced_block("json", &content)
  ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxNodeKind {
  Leaf,
  EmptyList,
  Expression,
}

impl std::fmt::Display for SyntaxNodeKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(match self {
      Self::Leaf => "leaf",
      Self::EmptyList => "empty-list",
      Self::Expression => "expression",
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSyntaxInput {
  pub node: Cirru,
  pub selected_format: SyntaxInputFormat,
  pub node_kind: SyntaxNodeKind,
}

impl DecodedSyntaxInput {
  pub fn canonical_json_ast(&self) -> serde_json::Value {
    cirru_to_json_value(&self.node)
  }

  #[cfg(test)]
  pub fn structured_summary(&self) -> serde_json::Value {
    serde_json::json!({
      "input_format": self.selected_format.to_string(),
      "node_kind": self.node_kind.to_string(),
      "canonical": self.canonical_json_ast(),
    })
  }
}

fn syntax_node_kind(node: &Cirru) -> SyntaxNodeKind {
  match node {
    Cirru::Leaf(_) => SyntaxNodeKind::Leaf,
    Cirru::List(items) if items.is_empty() => SyntaxNodeKind::EmptyList,
    Cirru::List(_) => SyntaxNodeKind::Expression,
  }
}

fn json_value_kind(value: &serde_json::Value) -> &'static str {
  match value {
    serde_json::Value::String(_) => "string leaf",
    serde_json::Value::Array(items) if items.is_empty() => "empty array",
    serde_json::Value::Array(_) => "array expression",
    serde_json::Value::Object(_) => "object",
    serde_json::Value::Number(_) => "number",
    serde_json::Value::Bool(_) => "boolean",
    serde_json::Value::Null => "null",
  }
}

fn strict_json_ast_value_to_cirru(value: &serde_json::Value) -> Result<Cirru, String> {
  match value {
    serde_json::Value::String(value) => Ok(Cirru::Leaf(Arc::from(value.as_str()))),
    serde_json::Value::Array(items) => items
      .iter()
      .map(strict_json_ast_value_to_cirru)
      .collect::<Result<Vec<_>, _>>()
      .map(Cirru::List),
    other => Err(format!(
      "Syntax input format `json-ast` expected node kind `string leaf or array`, received node kind `{}`.",
      json_value_kind(other)
    )),
  }
}

fn decode_json_ast(raw: &str, compatibility_mode: bool) -> Result<Cirru, String> {
  let value: serde_json::Value = serde_json::from_str(raw).map_err(|error| {
    format!("Syntax input format `json-ast` expected a JSON string leaf or array node, received invalid JSON: {error}")
  })?;
  if !matches!(value, serde_json::Value::String(_) | serde_json::Value::Array(_)) {
    return Err(format!(
      "Syntax input format `json-ast` expected node kind `string leaf or array`, received node kind `{}`.",
      json_value_kind(&value)
    ));
  }
  if compatibility_mode {
    json_value_to_cirru(&value)
  } else {
    strict_json_ast_value_to_cirru(&value)
  }
}

pub fn decode_syntax_input(raw: &str, requested_format: SyntaxInputFormat) -> Result<DecodedSyntaxInput, String> {
  let selected_format = match requested_format {
    SyntaxInputFormat::Auto if raw.trim().starts_with('[') => SyntaxInputFormat::JsonAst,
    SyntaxInputFormat::Auto => SyntaxInputFormat::Cirru,
    explicit => explicit,
  };
  let node = match selected_format {
    SyntaxInputFormat::Cirru => {
      if raw.contains('\t') {
        return Err(
          "Syntax input format `cirru` expected node kind `quoted syntax`, received node kind `invalid Cirru EDN`: input contains tab characters; Cirru requires spaces for indentation."
            .to_string(),
        );
      }
      parse_edn_quote(raw).map_err(|error| {
        format!("Syntax input format `cirru` expected node kind `quoted syntax`, received node kind `invalid Cirru EDN`: {error}")
      })?
    }
    SyntaxInputFormat::JsonAst => {
      if raw.trim().len() > 2000 {
        eprintln!("\n⚠️  Note: JSON AST input is very large ({} chars).", raw.trim().len());
        eprintln!("   For large definitions, consider using placeholders and submitting in segments.");
        eprintln!();
      }
      decode_json_ast(raw, requested_format == SyntaxInputFormat::Auto)?
    }
    SyntaxInputFormat::Auto => unreachable!("auto syntax input is resolved before decoding"),
  };
  Ok(DecodedSyntaxInput {
    node_kind: syntax_node_kind(&node),
    node,
    selected_format,
  })
}

pub fn print_decoded_syntax_input(input: &DecodedSyntaxInput) {
  println!("# Decoded syntax input\n");
  println!("- input format: `{}`", input.selected_format);
  println!("- node kind: `{}`\n", input.node_kind);
  match markdown_cirru_section(2, "Canonical Cirru syntax", &input.node, 0) {
    Ok(section) => println!("{section}"),
    Err(error) => println!("## Canonical Cirru syntax\n\n_Unable to render preview: {error}_"),
  }
  if input.selected_format == SyntaxInputFormat::JsonAst {
    match markdown_json_section(2, "Canonical JSON AST", &input.canonical_json_ast()) {
      Ok(section) => println!("\n{section}"),
      Err(error) => println!("\n## Canonical JSON AST\n\n_Unable to render preview: {error}_"),
    }
  }
}

pub fn decode_mutation_syntax_input(raw: &str, requested_format: SyntaxInputFormat) -> Result<Cirru, String> {
  let decoded = decode_syntax_input(raw, requested_format)?;
  if requested_format != SyntaxInputFormat::Auto {
    print_decoded_syntax_input(&decoded);
  }
  Ok(decoded.node)
}

pub fn format_path_with_separator(path: &[usize], separator: &str) -> String {
  path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(separator)
}

pub fn format_path(path: &[usize]) -> String {
  if path.is_empty() {
    "root".to_string()
  } else {
    format!("@{}", format_path_with_separator(path, "."))
  }
}

/// Return the deps.cirru path next to a selected calcit.cirru snapshot.
pub fn deps_path_for_snapshot(snapshot_path: &str) -> String {
  Path::new(snapshot_path)
    .parent()
    .unwrap_or_else(|| Path::new("."))
    .join("deps.cirru")
    .display()
    .to_string()
}

/// Read the package version owned by the dependency manifest next to a
/// Snapshot. Project versions moved out of `calcit.cirru`, so read-only tools
/// must not report the compatibility value still present in older snapshots.
pub fn package_version_for_snapshot(snapshot_path: &str) -> Result<Option<String>, String> {
  calcit::module_manifest_version(Path::new(snapshot_path).parent().unwrap_or_else(|| Path::new(".")))
}

/// Refuse to rewrite a Snapshot with a different Calcit release than the one
/// pinned by the adjacent deps.cirru. Snapshot serialization can evolve between
/// releases, so a newer global CLI must not silently produce data that the
/// project's pinned toolchain cannot read.
pub fn guard_snapshot_mutation_toolchain(snapshot_path: &str) -> Result<(), String> {
  let deps_path = deps_path_for_snapshot(snapshot_path);
  if !Path::new(&deps_path).exists() {
    return Ok(());
  }

  let content = fs::read_to_string(&deps_path).map_err(|error| format!("Failed to read {deps_path}: {error}"))?;
  let data = cirru_edn::parse(&content).map_err(|error| format!("Failed to parse {deps_path}: {error}"))?;
  let deps = data
    .view_map()
    .map_err(|error| format!("Invalid dependency manifest {deps_path}: {error}"))?;
  let declared = match deps.get_or_nil("calcit-version") {
    Edn::Str(version) if !version.trim().is_empty() => version.to_string(),
    Edn::Str(_) | Edn::Nil => return Ok(()),
    value => return Err(format!("Invalid :calcit-version in {deps_path}: expected a string, got {value}")),
  };
  semver::Version::parse(&declared).map_err(|error| format!("Invalid :calcit-version '{declared}' in {deps_path}: {error}"))?;
  let running = env!("CARGO_PKG_VERSION");
  if declared == running {
    return Ok(());
  }
  let quoted_deps_path = shell_quote(&deps_path);

  Err(format!(
    "Snapshot mutation blocked: {deps_path} pins Calcit {declared}, but the running CLI is {running}.\n\
     Re-run this edit with Calcit {declared}, or explicitly upgrade the project first with `caps {quoted_deps_path} upgrade --all`.\n\
     No changes were written to {snapshot_path}."
  ))
}

fn is_shell_sensitive_char(ch: char) -> bool {
  matches!(
    ch,
    '>' | '<' | '|' | '&' | ';' | '(' | ')' | '$' | '*' | '?' | '[' | ']' | '{' | '}' | '!' | '`'
  )
}

pub fn shell_quote(raw: &str) -> String {
  format!("'{}'", raw.replace('\'', "'\"'\"'"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionLookup {
  pub resolved: String,
  pub warning: Option<String>,
}

pub fn resolve_definition_lookup<'a, I>(
  namespace: &str,
  requested: &str,
  definitions: I,
  auto_correct: bool,
) -> Result<DefinitionLookup, String>
where
  I: IntoIterator<Item = &'a str>,
{
  let definition_names: Vec<&str> = definitions.into_iter().collect();

  if definition_names.contains(&requested) {
    return Ok(DefinitionLookup {
      resolved: requested.to_string(),
      warning: None,
    });
  }

  let shell_candidates: Vec<(&str, char)> = definition_names
    .into_iter()
    .filter_map(|candidate| {
      let rest = candidate.strip_prefix(requested)?;
      let next_char = rest.chars().next()?;
      if is_shell_sensitive_char(next_char) {
        Some((candidate, next_char))
      } else {
        None
      }
    })
    .collect();

  if shell_candidates.is_empty() {
    return Err(format!("Definition '{requested}' not found in namespace '{namespace}'"));
  }

  let mut lines = vec![format!("Definition '{requested}' not found in namespace '{namespace}'.")];
  lines.push("Possible cause: your shell may have interpreted part of the definition name before calcit received it.".to_string());
  lines.push("This often happens with characters like >, <, |, &, $, *, ?, (, or ).".to_string());

  if shell_candidates.len() == 1 {
    let (candidate, shell_char) = shell_candidates[0];
    lines.push(format!(
      "Detected a likely intended definition: '{candidate}' (the next character after '{requested}' is shell-sensitive: '{shell_char}')."
    ));
    lines.push(format!(
      "Try quoting the full target, for example: {}",
      shell_quote(&format!("{namespace}/{candidate}"))
    ));

    if auto_correct {
      lines.push(format!("Auto-correcting to '{candidate}' for this read-only command."));
      return Ok(DefinitionLookup {
        resolved: candidate.to_string(),
        warning: Some(lines.join("\n")),
      });
    }
  } else {
    let preview = shell_candidates
      .iter()
      .take(4)
      .map(|(candidate, _)| format!("'{candidate}'"))
      .collect::<Vec<_>>()
      .join(", ");
    lines.push(format!(
      "Found multiple shell-sensitive candidates starting with '{requested}': {preview}"
    ));
    lines.push(format!(
      "Quote the full target to disambiguate, for example: {}",
      shell_quote(&format!("{namespace}/{}", shell_candidates[0].0))
    ));
  }

  Err(lines.join("\n"))
}

pub fn print_cli_warning_block(message: &str) {
  let mut lines = message.lines();
  if let Some(first) = lines.next() {
    eprintln!("\n⚠️  Warning: {first}");
    for line in lines {
      eprintln!("   {line}");
    }
    eprintln!();
  }
}

#[derive(Clone, Copy)]
pub enum GlobalTempPathKind {
  ScratchCode,
  Snapshot,
}

pub fn global_temp_path_guidance(path: &str, kind: GlobalTempPathKind) -> Option<String> {
  let path_ref = Path::new(path);
  if !path_ref.is_absolute() || !(path_ref.starts_with("/tmp") || path_ref.starts_with("/private/tmp")) {
    return None;
  }

  Some(match kind {
    GlobalTempPathKind::ScratchCode => format!(
      "`{path}` is under a global temporary directory. For project-local Calcit scratch input, prefer `.calcit/snippets/<name>`; keep `.calcit/` in `.gitignore`. For one-off multi-line input, omit `--file`/`--code` and pipe stdin instead."
    ),
    GlobalTempPathKind::Snapshot => format!(
      "snapshot `{path}` is under a global temporary directory. Keep `calcit.cirru` at the project root so relative module paths and project-local files resolve from the intended base directory. Use `.calcit/snippets/` only for scratch files passed with `--file`, not for the project snapshot."
    ),
  })
}

pub fn warn_on_global_temp_path(path: &str) {
  if let Some(message) = global_temp_path_guidance(path, GlobalTempPathKind::ScratchCode) {
    print_cli_warning_block(&message);
  }
}

pub fn warn_on_global_temp_snapshot_path(path: &str) {
  if let Some(message) = global_temp_path_guidance(path, GlobalTempPathKind::Snapshot) {
    print_cli_warning_block(&message);
  }
}

pub fn emit_cli_output(content: &str, to_stderr: bool) {
  if to_stderr {
    eprint!("{content}");
    if !content.ends_with('\n') {
      eprintln!();
    }
  } else {
    super::stdout::cli_print!("{content}");
    if !content.ends_with('\n') {
      super::stdout::cli_println!();
    }
  }
}

/// Parse path string like "@2.1.0" or "2.1.0" to Vec<usize>
pub fn parse_path(path_str: &str) -> Result<Vec<usize>, String> {
  if path_str.is_empty() {
    return Ok(vec![]);
  }
  let cleaned = path_str.strip_prefix('@').unwrap_or(path_str);

  if cleaned.contains(',') {
    return Err(format!(
      "Invalid path '{path_str}': comma separator is no longer supported. Use dot-separated coordinates, e.g. '@2.1.0'."
    ));
  }

  cleaned
    .split('.')
    .map(|s| s.trim().parse::<usize>().map_err(|e| format!("Invalid path index '{s}': {e}")))
    .collect()
}

pub fn validate_input_sources(sources: &[bool]) -> Result<(), String> {
  if sources.iter().filter(|&&enabled| enabled).count() > 1 {
    Err(ERR_MULTIPLE_INPUT_SOURCES.to_string())
  } else {
    Ok(())
  }
}

/// Read code input from --file, --code, or stdin (fallback).
/// At most one input source should be provided.
/// If stdin has no data (EOF immediately), returns `None`.
pub fn read_code_input(file: &Option<String>, code: &Option<String>) -> Result<Option<String>, String> {
  let sources = [file.is_some(), code.is_some()];
  validate_input_sources(&sources)?;

  if let Some(path) = file {
    warn_on_global_temp_path(path);
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file '{path}': {e}"))?;
    Ok(Some(content.trim().to_string()))
  } else if let Some(s) = code {
    if s.contains('\n') {
      eprintln!("\n⚠️  Note: Inline code contains newlines. Multi-line code in shell can be error-prone.");
      eprintln!("   Consider writing to a temporary file and using --file instead.");
      eprintln!();
    }
    Ok(Some(s.trim().to_string()))
  } else {
    // Fallback to reading from stdin if no source is specified
    let mut buf = String::new();
    let bytes_read =
      std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf).map_err(|e| format!("Failed to read from stdin: {e}"))?;
    if bytes_read == 0 {
      Ok(None)
    } else {
      Ok(Some(buf.trim().to_string()))
    }
  }
}

/// Parse Cirru EDN input and extract the `quote` payload.
/// `cirru_edn::parse` already enforces that top-level expressions must be
/// prefixed with `quote` — bare leafs or unquoted forms produce parse errors.
///
/// `quote hello` → symbol leaf `Cirru::Leaf("hello")`
/// `quote |hello` → string leaf `Cirru::Leaf("|hello")`
/// `quote (store (:store reel))` → expression AST
fn parse_edn_quote(raw: &str) -> Result<Cirru, String> {
  let trimmed = raw.trim();

  if trimmed.is_empty() {
    return Err(
      "Input is empty. Please provide Cirru code prefixed with `quote` (e.g. `quote value`, `quote |text`, or `quote (expr ...)`)."
        .to_string(),
    );
  }

  // Parse with cirru_edn, then require the result itself to be quoted code.
  // Some valid EDN values (for example `do :string`) are not code payloads and
  // must not silently cross the CLI code/data boundary.
  let edn = cirru_edn::parse(trimmed).map_err(|e| {
    let msg = e.to_string();
    if msg.contains("invalid operator for edn") || msg.contains("invalid nodes for edn") || msg.contains("missing edn quote value") {
      format!(
        "{msg}\n\nHint: Cirru EDN input must be prefixed with `quote`, and `quote` must wrap exactly one AST node.\n  ✅ `quote my-symbol`     — symbol leaf\n  ✅ `quote |text`         — string leaf\n  ✅ `quote (expr ...)`    — expression\n  ❌ `my-symbol`           — bare leaf (missing quote)\n  ❌ `(expr ...)`          — bare expression (missing quote)"
      )
    } else {
      format!("Failed to parse Cirru EDN: {msg}")
    }
  })?;

  match edn {
    cirru_edn::Edn::Quote(payload) => Ok(payload),
    _ => Err(
      "Expected Cirru code prefixed with `quote`. Use `quote value` for a symbol leaf, `quote |text` for a string leaf, or `quote $ expr ...` for an expression."
        .to_string(),
    ),
  }
}

/// Parse multiple Cirru code nodes, requiring every top-level form to use its
/// own `quote` boundary. This keeps batch input unambiguous: both
/// `quote symbol`, `quote |string`, and `quote $ expr ...` each represent exactly one AST node.
pub fn parse_quoted_cirru_nodes(raw: &str) -> Result<Vec<Cirru>, String> {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return Err(
      "Input is empty. Provide one or more quoted nodes, for example `quote value`, `quote |text`, or `quote $ expr ...`.".to_string(),
    );
  }
  if raw.contains('\t') {
    return Err(
      "Input contains tab characters. Cirru requires spaces for indentation.\n\
       Please replace tabs with 2 spaces."
        .to_string(),
    );
  }

  let nodes = cirru_parser::parse(raw).map_err(|e| format!("Failed to parse Cirru: {e}"))?;
  nodes
    .into_iter()
    .enumerate()
    .map(|(index, node)| match node {
      Cirru::List(items) if matches!(items.first(), Some(Cirru::Leaf(head)) if &**head == "quote") => {
        if items.len() == 2 {
          Ok(items[1].clone())
        } else {
          Err(format!(
            "Quoted node {} must contain exactly one payload; use `quote value` for a symbol leaf, `quote |text` for a string leaf, or `quote $ expr ...` for an expression.",
            index + 1
          ))
        }
      }
      _ => Err(format!(
        "Node {} is missing the `quote` code/data boundary. Use `quote value` for a symbol leaf, `quote |text` for a string leaf, or `quote $ expr ...` for an expression.",
        index + 1
      )),
    })
    .collect()
}

/// Parse raw input string into a `Cirru` node.
/// Format auto-detection: if the trimmed input starts with `[`, it is treated as a JSON AST.
/// Otherwise, it is treated as Cirru EDN with `quote` prefix (e.g. `quote symbol`, `quote |text`, or `quote (expr ...)`).
///
/// Cirru text input MUST use `quote` prefix — the `cirru_edn` parser enforces this natively.
/// Use `--code` for inline text, `--file` for file input, or pipe via stdin.
pub fn parse_input_to_cirru(raw: &str) -> Result<Cirru, String> {
  decode_syntax_input(raw, SyntaxInputFormat::Auto).map(|decoded| decoded.node)
}

#[cfg(test)]
mod tests {
  use super::{
    GlobalTempPathKind, decode_syntax_input, format_path, format_path_with_separator, global_temp_path_guidance,
    guard_snapshot_mutation_toolchain, markdown_fenced_block, package_version_for_snapshot, parse_input_to_cirru, parse_path,
    parse_quoted_cirru_nodes, resolve_definition_lookup, shell_quote,
  };
  use crate::cli_args::SyntaxInputFormat;
  use cirru_parser::Cirru;
  use std::fs;
  use std::sync::atomic::{AtomicU64, Ordering};
  use std::time::{SystemTime, UNIX_EPOCH};

  static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

  fn mutation_guard_fixture(deps_content: Option<&str>) -> (std::path::PathBuf, std::path::PathBuf) {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let fixture_id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("calcit mutation guard-{}-{nonce}-{fixture_id}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let snapshot = dir.join("calcit.cirru");
    fs::write(&snapshot, "{}\n").unwrap();
    if let Some(content) = deps_content {
      fs::write(dir.join("deps.cirru"), content).unwrap();
    }
    (dir, snapshot)
  }

  fn leaf(value: &str) -> Cirru {
    Cirru::Leaf(value.into())
  }

  fn list(items: Vec<Cirru>) -> Cirru {
    Cirru::List(items)
  }

  #[test]
  fn markdown_fence_is_longer_than_embedded_backticks() {
    assert_eq!(
      markdown_fenced_block("cirru", "def x ``` value\n"),
      "````cirru\ndef x ``` value\n````\n"
    );
    assert_eq!(markdown_fenced_block("json", "[]"), "```json\n[]\n```\n");
  }

  #[test]
  fn quoted_input_distinguishes_symbol_string_and_expression() {
    assert_eq!(parse_input_to_cirru("quote value").unwrap(), leaf("value"));
    assert_eq!(parse_input_to_cirru("quote |value").unwrap(), leaf("|value"));
    assert_eq!(parse_input_to_cirru("quote $ inc 1").unwrap(), list(vec![leaf("inc"), leaf("1")]));
  }

  #[test]
  fn explicit_syntax_formats_distinguish_ambiguous_node_shapes() {
    let leaf = decode_syntax_input(r#""[]""#, SyntaxInputFormat::JsonAst).unwrap();
    assert_eq!(leaf.structured_summary()["node_kind"], "leaf");
    assert_eq!(leaf.structured_summary()["canonical"], serde_json::json!("[]"));

    let call = decode_syntax_input(r#"["inc","1"]"#, SyntaxInputFormat::JsonAst).unwrap();
    assert_eq!(call.structured_summary()["node_kind"], "expression");

    let empty = decode_syntax_input("[]", SyntaxInputFormat::JsonAst).unwrap();
    assert_eq!(empty.structured_summary()["node_kind"], "empty-list");

    let quoted = decode_syntax_input("quote $ []", SyntaxInputFormat::Cirru).unwrap();
    assert_eq!(quoted.structured_summary()["node_kind"], "expression");
    assert_eq!(quoted.structured_summary()["canonical"], serde_json::json!(["[]"]));
  }

  #[test]
  fn explicit_syntax_format_errors_report_expected_and_received_shapes() {
    let error = decode_syntax_input(r#"{"node":[]}"#, SyntaxInputFormat::JsonAst).unwrap_err();
    assert!(error.contains("format `json-ast`"), "error: {error}");
    assert!(error.contains("expected node kind `string leaf or array`"), "error: {error}");
    assert!(error.contains("received node kind `object`"), "error: {error}");

    let nested_scalar = decode_syntax_input("[1]", SyntaxInputFormat::JsonAst).unwrap_err();
    assert!(nested_scalar.contains("received node kind `number`"), "error: {nested_scalar}");

    assert_eq!(
      decode_syntax_input("[1]", SyntaxInputFormat::Auto).unwrap().node,
      list(vec![leaf("1")]),
      "compatibility auto mode must retain legacy JSON scalar conversion"
    );
  }

  #[test]
  fn code_input_rejects_non_quote_edn_values() {
    let error = parse_input_to_cirru("do :string").expect_err("plain EDN must not be accepted as code");
    assert!(error.contains("prefixed with `quote`"), "error: {error}");
  }

  #[test]
  fn code_input_rejects_quote_with_multiple_payloads() {
    let error = parse_input_to_cirru("quote println |hello").expect_err("quote must wrap one node");
    assert!(error.contains("exactly one AST node"), "error: {error}");
  }

  #[test]
  fn batch_quoted_input_preserves_leaf_and_expression_nodes() {
    assert_eq!(
      parse_quoted_cirru_nodes("quote $ inc 1\nquote |literal").unwrap(),
      vec![list(vec![leaf("inc"), leaf("1")]), leaf("|literal")]
    );
  }

  #[test]
  fn batch_quoted_input_rejects_ambiguous_bare_forms() {
    let error = parse_quoted_cirru_nodes("inc 1\nquote |literal").expect_err("bare expression must be rejected");
    assert!(error.contains("Node 1 is missing the `quote`"), "error: {error}");
  }

  #[test]
  fn rejects_comma_separated_paths() {
    let err = parse_path("3,2,1").unwrap_err();
    assert!(err.contains("comma separator is no longer supported"));
  }

  #[test]
  fn parses_dot_separated_paths() {
    assert_eq!(parse_path("3.2.1").unwrap(), vec![3, 2, 1]);
    assert_eq!(parse_path("@3.2.1").unwrap(), vec![3, 2, 1]);
  }

  #[test]
  fn rejects_mixed_separators() {
    assert!(parse_path("3,2.1").is_err());
  }

  #[test]
  fn formats_paths_with_dot_by_default() {
    assert_eq!(format_path(&[3, 2, 1]), "@3.2.1");

    assert_eq!(format_path_with_separator(&[3, 2, 1], ","), "3,2,1");
  }

  #[test]
  fn quotes_shell_targets_with_single_quotes() {
    assert_eq!(shell_quote("app.main/element->node"), "'app.main/element->node'");
  }

  #[test]
  fn global_tmp_path_guidance_points_to_project_local_snippets() {
    for path in ["/tmp/change.cirru", "/private/tmp/change.cirru"] {
      let guidance = global_temp_path_guidance(path, GlobalTempPathKind::ScratchCode).expect("global tmp path should be recognized");
      assert!(guidance.contains(".calcit/snippets/<name>"));
      assert!(guidance.contains("stdin"));
    }

    assert!(global_temp_path_guidance(".calcit/snippets/change.cirru", GlobalTempPathKind::ScratchCode).is_none());
    assert!(global_temp_path_guidance("/tmp-project/change.cirru", GlobalTempPathKind::ScratchCode).is_none());
  }

  #[test]
  fn global_tmp_snapshot_guidance_preserves_project_root() {
    let guidance = global_temp_path_guidance("/tmp/project/calcit.cirru", GlobalTempPathKind::Snapshot)
      .expect("global tmp snapshot should be recognized");
    assert!(guidance.contains("project root"));
    assert!(guidance.contains("relative module paths"));
    assert!(guidance.contains("not for the project snapshot"));
  }

  #[test]
  fn auto_corrects_unique_shell_truncated_definition() {
    let lookup = resolve_definition_lookup("respo.render.html", "element-", vec!["element->node", "render-app"], true).unwrap();

    assert_eq!(lookup.resolved, "element->node");
    let warning = lookup.warning.unwrap();
    assert!(warning.contains("Possible cause: your shell may have interpreted part of the definition name"));
    assert!(warning.contains("Auto-correcting to 'element->node'"));
  }

  #[test]
  fn keeps_plain_not_found_when_no_shell_candidate_exists() {
    let err = resolve_definition_lookup("app.main", "missing", vec!["main", "helper"], false).unwrap_err();
    assert_eq!(err, "Definition 'missing' not found in namespace 'app.main'");
  }

  #[test]
  fn reports_ambiguous_shell_truncated_definition() {
    let err = resolve_definition_lookup("app.main", "value-", vec!["value->text", "value->debug", "other"], false).unwrap_err();

    assert!(err.contains("Found multiple shell-sensitive candidates starting with 'value-'"));
    assert!(err.contains("'value->text'"));
    assert!(err.contains("'value->debug'"));
  }

  #[test]
  fn snapshot_mutation_guard_accepts_matching_or_unpinned_projects() {
    let matching = format!("{{}} (:calcit-version |{})\n", env!("CARGO_PKG_VERSION"));
    for deps in [Some(matching.as_str()), Some("{} (:dependencies $ {})\n"), None] {
      let (dir, snapshot) = mutation_guard_fixture(deps);
      guard_snapshot_mutation_toolchain(snapshot.to_str().unwrap()).unwrap();
      fs::remove_dir_all(dir).unwrap();
    }
  }

  #[test]
  fn package_version_comes_from_dependency_manifest() {
    let (dir, snapshot) = mutation_guard_fixture(Some("{} (:version |1.2.3) (:calcit-version |0.13.69)\n"));
    assert_eq!(
      package_version_for_snapshot(snapshot.to_str().unwrap()).unwrap(),
      Some("1.2.3".to_owned())
    );
    fs::remove_dir_all(dir).unwrap();
  }

  #[test]
  fn package_version_allows_legacy_or_unversioned_projects() {
    for deps in [Some("{} (:dependencies $ {})\n"), None] {
      let (dir, snapshot) = mutation_guard_fixture(deps);
      assert_eq!(package_version_for_snapshot(snapshot.to_str().unwrap()).unwrap(), None);
      fs::remove_dir_all(dir).unwrap();
    }
  }

  #[test]
  fn package_version_rejects_invalid_manifest_values() {
    let (dir, snapshot) = mutation_guard_fixture(Some("{} (:version 1)\n"));
    let error = package_version_for_snapshot(snapshot.to_str().unwrap()).unwrap_err();
    assert!(error.contains("Invalid :version"), "error: {error}");
    fs::remove_dir_all(dir).unwrap();
  }

  #[test]
  fn snapshot_mutation_guard_rejects_version_mismatch() {
    let (dir, snapshot) = mutation_guard_fixture(Some("{} (:calcit-version |0.1.0)\n"));
    let error = guard_snapshot_mutation_toolchain(snapshot.to_str().unwrap()).unwrap_err();
    assert!(error.contains("Snapshot mutation blocked"), "error: {error}");
    assert!(error.contains("pins Calcit 0.1.0"), "error: {error}");
    assert!(
      error.contains("caps '") && error.contains("deps.cirru' upgrade --all"),
      "error: {error}"
    );
    assert!(error.contains("No changes were written"), "error: {error}");
    fs::remove_dir_all(dir).unwrap();
  }

  #[test]
  fn snapshot_mutation_guard_rejects_invalid_declared_version() {
    let (dir, snapshot) = mutation_guard_fixture(Some("{} (:calcit-version |latest)\n"));
    let error = guard_snapshot_mutation_toolchain(snapshot.to_str().unwrap()).unwrap_err();
    assert!(error.contains("Invalid :calcit-version 'latest'"), "error: {error}");
    fs::remove_dir_all(dir).unwrap();
  }
}
