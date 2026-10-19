use crate::util::string::has_ns_part;

/// JavaScript reserved words that are valid Calcit symbols but cannot be used as JS binding names.
const JS_RESERVED_WORDS: &[&str] = &[
  "await",
  "break",
  "catch",
  "class",
  "const",
  "continue",
  "debugger",
  "default",
  "delete",
  "enum",
  "export",
  "extends",
  "finally",
  "for",
  "function",
  "implements",
  "import",
  "in",
  "instanceof",
  "interface",
  "package",
  "private",
  "protected",
  "public",
  "return",
  "static",
  "super",
  "switch",
  "this",
  "throw",
  "try",
  "typeof",
  "var",
  "void",
  "while",
  "with",
  "yield",
];

pub(super) fn escape_var(name: &str) -> String {
  if has_ns_part(name) {
    unreachable!("Invalid variable name `{}`, use `escape_ns_var` instead", name);
  }
  match name {
    "if" => String::from("_IF_"),
    "do" => String::from("_DO_"),
    "else" => String::from("_ELSE_"),
    "let" => String::from("_LET_"),
    "case" => String::from("_CASE_"),
    "new" => String::from("_NEW_"),
    "-" => String::from("_SUB_"),
    _ if JS_RESERVED_WORDS.contains(&name) => format!("_{}_", name.to_uppercase()),
    _ => name
      .replace('-', "_")
      .replace('.', "_DOT_")
      .replace('?', "_$q_")
      .replace('+', "_ADD_")
      .replace('^', "_CRT_")
      .replace('*', "_$s_")
      .replace('&', "_$n_")
      .replace("{}", "_$M_")
      .replace("[]", "_$L_")
      .replace('{', "_CURL_")
      .replace('}', "_CURR_")
      .replace('\'', "_SQUO_")
      .replace('[', "_SQRL_")
      .replace(']', "_SQRR_")
      .replace('!', "_$x_")
      .replace('%', "_PCT_")
      .replace('/', "_SLSH_")
      .replace('=', "_$e_")
      .replace('>', "_GT_")
      .replace('<', "_LT_")
      .replace(':', "_$o_")
      .replace(';', "_SCOL_")
      .replace('#', "_SHA_")
      .replace('\\', "_BSL_"),
  }
}

pub(super) fn escape_cirru_str(s: &str) -> String {
  let mut result = String::from('"');
  for c in s.chars() {
    match c {
      '\\' => result.push_str("\\\\"),
      '\"' => result.push_str("\\\""),
      '\n' => result.push_str("\\n"),
      '\r' => result.push_str("\\r"),
      '\u{2028}' => result.push_str("\\u2028"),
      '\u{2029}' => result.push_str("\\u2029"),
      '\t' => result.push_str("\\t"),
      _ => result.push(c),
    }
  }
  result.push('"');
  result
}

pub(crate) fn unescape_var(name: &str) -> String {
  match name {
    "_IF_" => return String::from("if"),
    "_DO_" => return String::from("do"),
    "_ELSE_" => return String::from("else"),
    "_LET_" => return String::from("let"),
    "_CASE_" => return String::from("case"),
    "_NEW_" => return String::from("new"),
    "_SUB_" => return String::from("-"),
    _ => {}
  }
  if let Some(inner) = name.strip_prefix('_').and_then(|n| n.strip_suffix('_')) {
    let lower = inner.to_lowercase();
    if inner == lower.to_uppercase() && JS_RESERVED_WORDS.contains(&lower.as_str()) {
      return lower;
    }
  }

  let mut decoded = name.to_string();
  for (from, to) in [
    ("_$q_", "?"),
    ("_ADD_", "+"),
    ("_CRT_", "^"),
    ("_$s_", "*"),
    ("_$n_", "&"),
    ("_$M_", "{}"),
    ("_$L_", "[]"),
    ("_CURL_", "{"),
    ("_CURR_", "}"),
    ("_SQUO_", "'"),
    ("_SQRL_", "["),
    ("_SQRR_", "]"),
    ("_$x_", "!"),
    ("_PCT_", "%"),
    ("_SLSH_", "/"),
    ("_$e_", "="),
    ("_GT_", ">"),
    ("_LT_", "<"),
    ("_$o_", ":"),
    ("_SCOL_", ";"),
    ("_SHA_", "#"),
    ("_BSL_", "\\"),
    ("_DOT_", "."),
  ] {
    decoded = decoded.replace(from, to);
  }

  decoded
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn escapes_keywords_and_symbols() {
    assert_eq!(escape_var("if"), "_IF_");
    assert_eq!(escape_var("new"), "_NEW_");
    assert_eq!(escape_var("a-b?"), "a_b_$q_");
    assert_eq!(escape_var("[]"), "_$L_");
    assert_eq!(escape_var("default"), "_DEFAULT_");
    assert_eq!(escape_var("class"), "_CLASS_");
    assert_eq!(escape_var("default-value"), "default_value");
  }

  #[test]
  fn escapes_cirru_string_chars() {
    assert_eq!(escape_cirru_str("a\nb\t\\\""), "\"a\\nb\\t\\\\\\\"\"");
    assert_eq!(escape_cirru_str("\r\n\u{2028}\u{2029}\\r"), "\"\\r\\n\\u2028\\u2029\\\\r\"");
  }

  #[test]
  fn unescapes_symbols_best_effort() {
    assert_eq!(unescape_var("a_b_$q_"), "a_b?");
    assert_eq!(unescape_var("_DOT_"), ".");
    assert_eq!(unescape_var("_IF_"), "if");
    assert_eq!(unescape_var("_NEW_"), "new");
    assert_eq!(unescape_var("_DEFAULT_"), "default");
    assert_eq!(unescape_var("_Default_"), "_Default_");
  }
}
