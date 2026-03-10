//! Basic Cirru syntax validation for leaf nodes
//!
//! This module provides simple validation to catch common mistakes in Cirru leaf nodes:
//! - Tags (`:tag`) should not contain spaces
//! - Strings should start with `|` or `"`
//! - Numbers should be parseable as valid numeric formats
//! - Leaf nodes with spaces that are not strings are suspicious

use cirru_parser::Cirru;
use std::sync::Arc;

/// Validates basic Cirru syntax rules for leaf nodes
pub fn validate_cirru_syntax(node: &Cirru) -> Result<(), String> {
  validate_node_recursive(node, &mut vec![], false)
}

fn validate_node_recursive(node: &Cirru, path: &mut Vec<usize>, in_comment: bool) -> Result<(), String> {
  match node {
    Cirru::Leaf(s) => validate_leaf(s, path, in_comment)?,
    Cirru::List(items) => {
      // Check if this list is a comment (starts with `;`)
      let is_comment = if let Some(Cirru::Leaf(first)) = items.first() {
        first.as_ref() == ";"
      } else {
        false
      };

      for (idx, item) in items.iter().enumerate() {
        path.push(idx);
        // Pass comment context to children (skip first element which is `;` itself)
        let child_in_comment = is_comment && idx > 0;
        validate_node_recursive(item, path, child_in_comment)?;
        path.pop();
      }
    }
  }
  Ok(())
}

fn validate_leaf(s: &Arc<str>, path: &[usize], in_comment: bool) -> Result<(), String> {
  let text = s.as_ref();

  // Empty leaf is allowed
  if text.is_empty() {
    return Ok(());
  }

  // Tags starting with `:` should not contain spaces
  if text.starts_with(':') {
    if text.contains(' ') {
      return Err(format!(
        "Invalid tag at path [{}]: Tags cannot contain spaces\n\
         Found: {:?}\n\
         Hint: Tags like :tag should be single tokens without spaces",
        format_path(path),
        text
      ));
    }
    // Valid tag
    return Ok(());
  }

  // Symbols starting with `'` should not contain spaces
  if text.starts_with('\'') {
    if text.contains(' ') {
      return Err(format!(
        "Invalid symbol at path [{}]: Symbols cannot contain spaces\n\
         Found: {:?}\n\
         Hint: Symbols like 'atom should be single tokens without spaces",
        format_path(path),
        text
      ));
    }
    // Valid symbol
    return Ok(());
  }

  // Strings starting with `|` or `"` are valid
  if text.starts_with('|') || text.starts_with('"') {
    return Ok(());
  }

  // Comments starting with `;` can contain spaces (Cirru comment syntax)
  if text.starts_with(';') {
    return Ok(());
  }

  // Check for invalid characters in non-string nodes: ( ) should not appear
  // These are structural characters in Cirru and should never be in leaf content
  if text.contains('(') || text.contains(')') {
    return Err(format!(
      "Invalid leaf node at path [{}]: Contains parentheses which are structural characters\n\
       Found: {:?}\n\
       Hint: Parentheses ( ) are only for list structure, not leaf content",
      format_path(path),
      text
    ));
  }

  // If we're inside a comment, skip all semantic validation (spaces, numbers, tags, symbols)
  // Only basic Cirru syntax is enforced (like no parentheses above)
  if in_comment {
    return Ok(());
  }

  // Check if it looks like a number (digit, or +/- followed by digit)
  let first_char = text.chars().next().unwrap();
  if first_char.is_ascii_digit() {
    // Starts with digit, must be a valid number
    if !is_valid_number(text) {
      return Err(format!(
        "Invalid number format at path [{}]: Starts with digit but cannot be parsed as number\n\
         Found: {:?}\n\
         Hint: Valid formats include: 123, -456, 3.14, 1e10, 0x1F, 0b1010, 0o77",
        format_path(path),
        text
      ));
    }
    return Ok(());
  }

  // +/- followed by digit should be a valid number
  if (first_char == '+' || first_char == '-') && text.len() > 1 {
    let second_char = text.chars().nth(1).unwrap();
    if second_char.is_ascii_digit() {
      // Looks like a signed number, validate it
      if !is_valid_number(text) {
        return Err(format!(
          "Invalid number format at path [{}]: Starts with {}{} but cannot be parsed as number\n\
           Found: {:?}\n\
           Hint: Valid formats include: +123, -456, +3.14, -1e10",
          format_path(path),
          first_char,
          second_char,
          text
        ));
      }
      return Ok(());
    }
    // Otherwise, +/- alone or followed by non-digit is a valid symbol
  }

  // Check for suspicious patterns: leaf contains spaces but is not a string
  if text.contains(' ') {
    return Err(format!(
      "Suspicious leaf node at path [{}]: Contains spaces but is not a string\n\
       Found: {:?}\n\
       Hint: If this is meant to be a string, prefix with | or \"\n\
       If it's multiple tokens, it should be a list (separate expressions)",
      format_path(path),
      text
    ));
  }

  // Symbol/identifier - can contain special chars like -, ?, !, +, *, /, <, >, =, &, $, %
  // This is valid as long as it doesn't contain spaces
  Ok(())
}

fn is_valid_number(text: &str) -> bool {
  // Try to parse as various number formats

  // Integer (decimal, hex, binary, octal)
  if text.parse::<i64>().is_ok() || text.parse::<u64>().is_ok() {
    return true;
  }

  // Hex: 0x1F, 0X1F
  if text.len() > 2 && (text.starts_with("0x") || text.starts_with("0X")) && text[2..].chars().all(|c| c.is_ascii_hexdigit()) {
    return true;
  }

  // Binary: 0b1010, 0B1010
  if text.len() > 2 && (text.starts_with("0b") || text.starts_with("0B")) && text[2..].chars().all(|c| c == '0' || c == '1') {
    return true;
  }

  // Octal: 0o77, 0O77
  if text.len() > 2 && (text.starts_with("0o") || text.starts_with("0O")) && text[2..].chars().all(|c| c.is_ascii_digit() && c < '8') {
    return true;
  }
  // Float: 3.14, -2.5, 1e10, 1.5e-3
  if text.parse::<f64>().is_ok() {
    return true;
  }

  // Check for scientific notation patterns that f64::parse might miss
  if text.contains('e') || text.contains('E') {
    let parts: Vec<&str> = if text.contains('e') {
      text.split('e').collect()
    } else {
      text.split('E').collect()
    };

    if parts.len() == 2 {
      let base_valid = parts[0].parse::<f64>().is_ok();
      let exp_valid = parts[1].parse::<i32>().is_ok();
      if base_valid && exp_valid {
        return true;
      }
    }
  }

  false
}

fn format_path(path: &[usize]) -> String {
  if path.is_empty() {
    "root".to_string()
  } else {
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn leaf(s: &str) -> Cirru {
    Cirru::Leaf(Arc::from(s))
  }

  fn list(items: Vec<Cirru>) -> Cirru {
    Cirru::List(items)
  }

  #[test]
  fn test_valid_tags() {
    assert!(validate_cirru_syntax(&leaf(":tag")).is_ok());
    assert!(validate_cirru_syntax(&leaf(":event/click")).is_ok());
    assert!(validate_cirru_syntax(&leaf(":ns/def")).is_ok());
  }

  #[test]
  fn test_invalid_tags_with_spaces() {
    let result = validate_cirru_syntax(&leaf(":tag with space"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Tags cannot contain spaces"));
  }

  #[test]
  fn test_valid_quoted_symbols() {
    assert!(validate_cirru_syntax(&leaf("'atom")).is_ok());
    assert!(validate_cirru_syntax(&leaf("'my-symbol")).is_ok());
    assert!(validate_cirru_syntax(&leaf("'x")).is_ok());
  }

  #[test]
  fn test_invalid_symbols_with_spaces() {
    let result = validate_cirru_syntax(&leaf("'a b"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Symbols cannot contain spaces"));
  }

  #[test]
  fn test_invalid_parentheses_in_leaves() {
    let result1 = validate_cirru_syntax(&leaf("hello(world)"));
    assert!(result1.is_err());
    assert!(result1.unwrap_err().contains("parentheses"));

    let result2 = validate_cirru_syntax(&leaf("(bad)"));
    assert!(result2.is_err());

    let result3 = validate_cirru_syntax(&leaf("test)"));
    assert!(result3.is_err());
  }

  #[test]
  fn test_valid_strings_with_parentheses() {
    // Strings can contain parentheses
    assert!(validate_cirru_syntax(&leaf("|hello (world)")).is_ok());
    assert!(validate_cirru_syntax(&leaf("\"text (with) parens\"")).is_ok());
  }

  #[test]
  fn test_comments_allow_flexible_content() {
    // Comments (starting with `;`) can contain spaces and other content
    let comment_list = list(vec![leaf(";"), leaf("this is a comment")]);
    assert!(validate_cirru_syntax(&comment_list).is_ok());

    // Even invalid-looking content like numbers with colons are OK in comments
    let comment_with_weird_number = list(vec![leaf(";"), leaf("测试 1: something")]);
    assert!(validate_cirru_syntax(&comment_with_weird_number).is_ok());

    // But parentheses are still not allowed even in comments (Cirru syntax rule)
    let comment_with_parens = list(vec![leaf(";"), leaf("bad (comment)")]);
    assert!(validate_cirru_syntax(&comment_with_parens).is_err());
  }

  #[test]
  fn test_valid_strings() {
    assert!(validate_cirru_syntax(&leaf("|hello world")).is_ok());
    assert!(validate_cirru_syntax(&leaf("\"hello world\"")).is_ok());
    assert!(validate_cirru_syntax(&leaf("|text with spaces")).is_ok());
  }

  #[test]
  fn test_valid_numbers() {
    assert!(validate_cirru_syntax(&leaf("123")).is_ok());
    assert!(validate_cirru_syntax(&leaf("-456")).is_ok());
    assert!(validate_cirru_syntax(&leaf("3.14")).is_ok());
    assert!(validate_cirru_syntax(&leaf("1e10")).is_ok());
    assert!(validate_cirru_syntax(&leaf("1.5e-3")).is_ok());
    assert!(validate_cirru_syntax(&leaf("0x1F")).is_ok());
    assert!(validate_cirru_syntax(&leaf("0b1010")).is_ok());
    assert!(validate_cirru_syntax(&leaf("0o77")).is_ok());
  }

  #[test]
  fn test_invalid_numbers() {
    let result = validate_cirru_syntax(&leaf("123abc"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot be parsed as number"));
  }

  #[test]
  fn test_valid_symbols() {
    assert!(validate_cirru_syntax(&leaf("defn")).is_ok());
    assert!(validate_cirru_syntax(&leaf("valid?")).is_ok());
    assert!(validate_cirru_syntax(&leaf("add!")).is_ok());
    assert!(validate_cirru_syntax(&leaf("+")).is_ok());
    assert!(validate_cirru_syntax(&leaf("->")).is_ok());
    assert!(validate_cirru_syntax(&leaf("&+")).is_ok());
  }

  #[test]
  fn test_suspicious_leaf_with_spaces() {
    let result = validate_cirru_syntax(&leaf("hello world"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Contains spaces but is not a string"));
  }

  #[test]
  fn test_valid_nested_structure() {
    let tree = list(vec![
      leaf("defn"),
      leaf("add"),
      list(vec![leaf("a"), leaf("b")]),
      list(vec![leaf("+"), leaf("a"), leaf("b")]),
    ]);
    assert!(validate_cirru_syntax(&tree).is_ok());
  }

  #[test]
  fn test_invalid_nested_structure() {
    let tree = list(vec![
      leaf("defn"),
      leaf("my func"), // Invalid: space without string prefix
      list(vec![leaf("a"), leaf("b")]),
    ]);
    let result = validate_cirru_syntax(&tree);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Contains spaces but is not a string"));
  }
}
