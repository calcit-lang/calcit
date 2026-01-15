//! Common utilities shared between CLI handlers

use cirru_parser::Cirru;
use std::fs;
use std::io::{self, Read};
use std::sync::Arc;

// Error message constants
pub const ERR_MULTIPLE_INPUT_SOURCES: &str =
  "Multiple input sources provided. Use only one of: --stdin/-s, --file/-f, --code/-e, or --json/-j.";

pub const ERR_CONFLICTING_INPUT_FLAGS: &str = "Conflicting input flags: --leaf cannot be used with --json-input.";

pub const ERR_CODE_INPUT_REQUIRED: &str = "Code input required: use --file, --code, --json, or --stdin";

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

/// Parse path string like "2,1,0" to Vec<usize>
pub fn parse_path(path_str: &str) -> Result<Vec<usize>, String> {
  if path_str.is_empty() {
    return Ok(vec![]);
  }

  path_str
    .split(',')
    .map(|s| s.trim().parse::<usize>().map_err(|e| format!("Invalid path index '{s}': {e}")))
    .collect()
}

/// Validate input source conflicts
pub fn validate_input_sources(sources: &[bool]) -> Result<(), String> {
  let count = sources.iter().filter(|&&x| x).count();
  if count > 1 {
    return Err(ERR_MULTIPLE_INPUT_SOURCES.to_string());
  }
  Ok(())
}

/// Validate input flag conflicts
pub fn validate_input_flags(leaf_input: bool, json_input: bool) -> Result<(), String> {
  if leaf_input && json_input {
    return Err(ERR_CONFLICTING_INPUT_FLAGS.to_string());
  }
  Ok(())
}

/// Read code input from file, inline code, json option, or stdin.
/// Exactly one input source should be used.
pub fn read_code_input(
  file: &Option<String>,
  code: &Option<String>,
  json: &Option<String>,
  stdin: bool,
) -> Result<Option<String>, String> {
  let sources = [stdin, file.is_some(), code.is_some(), json.is_some()];
  validate_input_sources(&sources)?;

  if stdin {
    let mut buffer = String::new();
    io::stdin()
      .read_to_string(&mut buffer)
      .map_err(|e| format!("Failed to read from stdin: {e}"))?;
    Ok(Some(buffer.trim().to_string()))
  } else if let Some(path) = file {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file '{path}': {e}"))?;
    Ok(Some(content.trim().to_string()))
  } else if let Some(s) = code {
    Ok(Some(s.trim().to_string()))
  } else if let Some(j) = json {
    Ok(Some(j.clone()))
  } else {
    Ok(None)
  }
}

/// Check if a Cirru node is a single-element list containing only a string leaf,
/// which might confuse LLM thinking it's a leaf node when it's actually an expression.
pub fn warn_if_single_string_expression(node: &Cirru, input_source: &str) {
  if let Cirru::List(items) = node {
    if items.len() == 1 {
      if let Some(Cirru::Leaf(_)) = items.first() {
        eprintln!("\n⚠️  Note: Cirru one-liner input '{input_source}' was parsed as an expression (list with one element).");
        eprintln!("   In Cirru syntax, this creates a list containing one element.");
        eprintln!("   If you want a leaf node (plain string), use --leaf parameter.");
        eprintln!("   Example: --leaf -e '{input_source}' creates a leaf, not an expression.\n");
      }
    }
  }
}

/// Determine input mode and parse raw input string into a `Cirru` node.
/// Precedence (highest to lowest):
/// - `--json <string>` (inline JSON)
/// - `--leaf` (treat raw input as a Cirru leaf)
/// - `--json-input` (parse JSON -> Cirru)
/// - Cirru one-liner (default)
pub fn parse_input_to_cirru(
  raw: &str,
  inline_json: &Option<String>,
  json_input: bool,
  leaf: bool,
  auto_json: bool,
) -> Result<Cirru, String> {
  // Validate conflicting flags early (keep error messages user-friendly)
  validate_input_flags(leaf, json_input)?;

  // If inline JSON provided, use it (takes precedence)
  if let Some(j) = inline_json {
    let node = json_to_cirru(j)?;
    if leaf {
      match node {
        Cirru::Leaf(_) => Ok(node),
        _ => Err("--leaf expects a JSON string (leaf node), but got a non-leaf JSON value.".to_string()),
      }
    } else {
      Ok(node)
    }
  } else if leaf {
    // --leaf: automatically treat raw input as a Cirru leaf node
    Ok(Cirru::Leaf(Arc::from(raw)))
  } else if json_input {
    json_to_cirru(raw)
  } else {
    // If input comes from inline `--code/-e`, it's typically single-line.
    // Auto-detect JSON arrays/strings so users don't need `-J` for inline JSON.
    if auto_json {
      let trimmed = raw.trim();
      let looks_like_json_string = trimmed.starts_with('"') && trimmed.ends_with('"');
      // Heuristic for `-e/--code`:
      // - If it is a JSON string: starts/ends with quotes -> JSON
      // - If it is a JSON array: starts with '[' and ends with ']' AND contains at least one '"' -> JSON
      //   (This avoids ambiguity with Cirru list syntax like `[]` or `[] 1 2 3`.)
      let looks_like_json_array = trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.contains('"');

      // If it looks like JSON, treat it as JSON.
      // Do NOT fall back to Cirru one-liner on JSON parse failure, otherwise invalid JSON
      // can be silently accepted as a Cirru expression.
      if looks_like_json_array || looks_like_json_string {
        return json_to_cirru(trimmed).map_err(|e| format!("Failed to parse JSON from -e/--code: {e}"));
      }

      // Inline `-e/--code` defaults to Cirru one-liner expr when it's not JSON.
      if trimmed.is_empty() {
        return Err("Input is empty. Please provide Cirru code or use -j for JSON input.".to_string());
      }
      if raw.contains('\t') {
        return Err(
          "Input contains tab characters. Cirru requires spaces for indentation.\n\
           Please replace tabs with 2 spaces.\n\
           Tip: Use `cat -A file` to check for tabs (shown as ^I)."
            .to_string(),
        );
      }

      let result = cirru_parser::parse_expr_one_liner(raw).map_err(|e| format!("Failed to parse Cirru one-liner expression: {e}"))?;
      warn_if_single_string_expression(&result, raw);
      return Ok(result);
    }

    // Check for common mistakes before parsing
    let trimmed = raw.trim();

    // Check for empty input
    if trimmed.is_empty() {
      return Err("Input is empty. Please provide Cirru code or use -j for JSON input.".to_string());
    }

    // Detect JSON input without --json-input flag
    // JSON arrays look like: ["item", ...] or [ "item", ...]
    // Cirru [] syntax looks like: [] 1 2 3 or []
    // Key difference: JSON has ["..." at start, Cirru has [] followed by space or newline
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
      // Check if it looks like JSON (starts with [" or [ ")
      let after_bracket = &trimmed[1..];
      let is_likely_json = after_bracket.starts_with('"')
        || after_bracket.starts_with(' ') && after_bracket.trim_start().starts_with('"')
        || after_bracket.starts_with('\n') && after_bracket.trim_start().starts_with('"');

      // Also check: Cirru [] is followed by space then non-quote content
      let is_cirru_list = after_bracket.starts_with(']') // empty []
      || (after_bracket.starts_with(' ') && !after_bracket.trim_start().starts_with('"'));

      if is_likely_json && !is_cirru_list {
        return Err(
          "Input appears to be JSON format (starts with '[\"').\n\
         If you want to use JSON input, use one of:\n\
         - inline JSON: cr edit def ns/name -j '[\"defn\", ...]'\n\
         - inline code: cr edit def ns/name -e '[\"defn\", ...]'\n\
         - file/stdin JSON: add -J or --json-input (e.g. -f code.json -J, or -s -J).\n\
         Note: Cirru's [] list syntax (e.g. '[] 1 2 3') is different and will be parsed correctly."
            .to_string(),
        );
      }
    }

    // Detect tabs in input
    if raw.contains('\t') {
      return Err(
        "Input contains tab characters. Cirru requires spaces for indentation.\n\
       Please replace tabs with 2 spaces.\n\
       Tip: Use `cat -A file` to check for tabs (shown as ^I)."
          .to_string(),
      );
    }

    // Default: parse as cirru text
    let parsed = cirru_parser::parse(raw).map_err(|e| {
      let err_str = e.to_string();
      let mut msg = format!("Failed to parse Cirru text: {err_str}");

      // Provide specific hints based on error type
      if err_str.contains("odd indentation") {
        msg.push_str("\n\nCirru requires 2-space indentation. Each nesting level must use exactly 2 spaces.");
        msg.push_str("\nExample:\n  defn my-fn (x)\n    &+ x 1");
      } else if err_str.contains("unexpected end of file") {
        msg.push_str("\n\nPossible cause: missing closing quotes or unclosed structural pattern.");
      }
      msg
    })?;

    // Return the expressions
    if parsed.len() == 1 {
      let result = parsed.into_iter().next().unwrap();
      warn_if_single_string_expression(&result, raw);
      Ok(result)
    } else if parsed.is_empty() {
      Err("Input parsed as an empty Cirru structure.".to_string())
    } else {
      Ok(Cirru::List(parsed))
    }
  }
}
