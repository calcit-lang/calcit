//! Common utilities shared between CLI handlers

use cirru_parser::Cirru;
use std::fs;
use std::path::Path;
use std::sync::Arc;

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

/// Convert JSON string to Cirru syntax tree
pub fn json_to_cirru(json_str: &str) -> Result<Cirru, String> {
  let json_value: serde_json::Value = serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {e}"))?;
  json_value_to_cirru(&json_value)
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
      "snapshot `{path}` is under a global temporary directory. Keep `calcit.cirru` (or legacy `compact.cirru`) at the project root so relative module paths and project-local files resolve from the intended base directory. Use `.calcit/snippets/` only for scratch files passed with `--file`, not for the project snapshot."
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
    print!("{content}");
    if !content.ends_with('\n') {
      println!();
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
  let trimmed = raw.trim();
  // Auto-detect: JSON arrays start with `[`, Cirru EDN starts with `quote`
  let is_json = trimmed.starts_with('[');
  if is_json {
    if trimmed.len() > 2000 {
      eprintln!("\n⚠️  Note: JSON input is very large ({} chars).", trimmed.len());
      eprintln!("   For large definitions, consider using placeholders and submitting in segments.");
      eprintln!();
    }
    json_to_cirru(trimmed)
  } else {
    // Parse as Cirru EDN with `quote` prefix
    if raw.contains('\t') {
      return Err(
        "Input contains tab characters. Cirru requires spaces for indentation.\n\
         Please replace tabs with 2 spaces.\n\
         Tip: Use `cat -A file` to check for tabs (shown as ^I)."
          .to_string(),
      );
    }

    parse_edn_quote(raw)
  }
}

#[cfg(test)]
mod tests {
  use super::{
    GlobalTempPathKind, format_path, format_path_with_separator, global_temp_path_guidance, parse_input_to_cirru, parse_path,
    parse_quoted_cirru_nodes, resolve_definition_lookup, shell_quote,
  };
  use cirru_parser::Cirru;

  fn leaf(value: &str) -> Cirru {
    Cirru::Leaf(value.into())
  }

  fn list(items: Vec<Cirru>) -> Cirru {
    Cirru::List(items)
  }

  #[test]
  fn quoted_input_distinguishes_symbol_string_and_expression() {
    assert_eq!(parse_input_to_cirru("quote value").unwrap(), leaf("value"));
    assert_eq!(parse_input_to_cirru("quote |value").unwrap(), leaf("|value"));
    assert_eq!(parse_input_to_cirru("quote $ inc 1").unwrap(), list(vec![leaf("inc"), leaf("1")]));
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
}
