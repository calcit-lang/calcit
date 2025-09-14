//! Validation utilities for namespace and definition names

use std::collections::HashSet;

/// Validates namespace names according to Calcit naming rules
///
/// Namespace names can contain:
/// - Letters (a-z, A-Z)
/// - Numbers (0-9)
/// - Special characters: -_$.
///
/// Forbidden characters: /~@ and other special symbols
pub fn validate_namespace_name(name: &str) -> Result<(), String> {
  if name.is_empty() {
    return Err("Namespace name cannot be empty".to_string());
  }

  // Check for forbidden characters
  let forbidden_chars: HashSet<char> = [
    '/', '~', '@', ' ', '\t', '\n', '\r', '(', ')', '[', ']', '{', '}', '<', '>', '"', '\'', '`', '|', '\\', ';', ',', '!', '#', '%',
    '&', '*', '+', ':', '?',
  ]
  .iter()
  .cloned()
  .collect();

  for ch in name.chars() {
    if forbidden_chars.contains(&ch) {
      return Err(format!(
        "Namespace name '{name}' contains forbidden character '{ch}'. Allowed characters: letters, numbers, and -_$."
      ));
    }

    // Only allow alphanumeric and specific special characters
    if !ch.is_alphanumeric() && !matches!(ch, '-' | '_' | '$' | '.') {
      return Err(format!(
        "Namespace name '{name}' contains invalid character '{ch}'. Allowed characters: letters, numbers, and -_$."
      ));
    }
  }

  // Additional validation rules
  if name.starts_with('.') || name.ends_with('.') {
    return Err(format!("Namespace name '{name}' cannot start or end with a dot"));
  }

  if name.contains("..") {
    return Err(format!("Namespace name '{name}' cannot contain consecutive dots"));
  }

  Ok(())
}

/// Validates definition names according to Calcit naming rules
///
/// Definition names can contain:
/// - Letters (a-z, A-Z)
/// - Numbers (0-9)
/// - Special characters: !#%&*-_+:?
///
/// Forbidden characters: /~@ and other special symbols
pub fn validate_definition_name(name: &str) -> Result<(), String> {
  if name.is_empty() {
    return Err("Definition name cannot be empty".to_string());
  }

  // Check for forbidden characters
  let forbidden_chars: HashSet<char> = [
    '/', '~', '@', ' ', '\t', '\n', '\r', '(', ')', '[', ']', '{', '}', '<', '>', '"', '\'', '`', '|', '\\', ';', ',', '.', '$',
  ]
  .iter()
  .cloned()
  .collect();

  for ch in name.chars() {
    if forbidden_chars.contains(&ch) {
      return Err(format!(
        "Definition name '{name}' contains forbidden character '{ch}'. Allowed characters: letters, numbers, and !#%&*-_+:?"
      ));
    }

    // Only allow alphanumeric and specific special characters
    if !ch.is_alphanumeric() && !matches!(ch, '!' | '#' | '%' | '&' | '*' | '-' | '_' | '+' | ':' | '?') {
      return Err(format!(
        "Definition name '{name}' contains invalid character '{ch}'. Allowed characters: letters, numbers, and !#%&*-_+:?"
      ));
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_valid_namespace_names() {
    assert!(validate_namespace_name("app").is_ok());
    assert!(validate_namespace_name("app.core").is_ok());
    assert!(validate_namespace_name("my-app.utils_v2").is_ok());
    assert!(validate_namespace_name("test$module").is_ok());
    assert!(validate_namespace_name("app123.module_test").is_ok());
  }

  #[test]
  fn test_invalid_namespace_names() {
    assert!(validate_namespace_name("").is_err());
    assert!(validate_namespace_name("app/core").is_err());
    assert!(validate_namespace_name("app~test").is_err());
    assert!(validate_namespace_name("app@domain").is_err());
    assert!(validate_namespace_name(".app").is_err());
    assert!(validate_namespace_name("app.").is_err());
    assert!(validate_namespace_name("app..core").is_err());
    assert!(validate_namespace_name("app core").is_err());
    assert!(validate_namespace_name("app!test").is_err());
  }

  #[test]
  fn test_valid_definition_names() {
    assert!(validate_definition_name("add").is_ok());
    assert!(validate_definition_name("add-numbers").is_ok());
    assert!(validate_definition_name("test_fn").is_ok());
    assert!(validate_definition_name("main!").is_ok());
    assert!(validate_definition_name("valid?").is_ok());
    assert!(validate_definition_name("*global*").is_ok());
    assert!(validate_definition_name("config:dev").is_ok());
    assert!(validate_definition_name("fn#123").is_ok());
    assert!(validate_definition_name("test%").is_ok());
    assert!(validate_definition_name("&rest").is_ok());
    assert!(validate_definition_name("+version+").is_ok());
  }

  #[test]
  fn test_invalid_definition_names() {
    assert!(validate_definition_name("").is_err());
    assert!(validate_definition_name("add/sub").is_err());
    assert!(validate_definition_name("test~fn").is_err());
    assert!(validate_definition_name("user@domain").is_err());
    assert!(validate_definition_name("test fn").is_err());
    assert!(validate_definition_name("app.core").is_err());
    assert!(validate_definition_name("test$var").is_err());
    assert!(validate_definition_name("fn()").is_err());
    assert!(validate_definition_name("test[0]").is_err());
  }
}
