use base64::Engine;
use serde_json::json;

pub(super) struct FfiLineMapping {
  pub generated_line: usize,
  pub original_line: usize,
  pub original_first_column: usize,
  pub code: String,
  pub source_name: String,
  pub source_content: String,
}

pub(super) fn js_line_slices(value: &str) -> Vec<&str> {
  let mut lines = Vec::new();
  let mut characters = value.char_indices().peekable();
  let mut start = 0;
  while let Some((index, character)) = characters.next() {
    let end = match character {
      '\r' => {
        if characters.peek().is_some_and(|(_, next)| *next == '\n') {
          characters.next();
          index + 2
        } else {
          index + 1
        }
      }
      '\n' => index + 1,
      '\u{2028}' | '\u{2029}' => index + character.len_utf8(),
      _ => continue,
    };
    lines.push(&value[start..index]);
    start = end;
  }
  lines.push(&value[start..]);
  lines
}

fn vlq(mut value: i64) -> String {
  const DIGITS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let negative = value < 0;
  if negative {
    value = -value;
  }
  let mut bits = ((value as u64) << 1) | u64::from(negative);
  let mut result = String::new();
  loop {
    let mut digit = (bits & 31) as u8;
    bits >>= 5;
    if bits != 0 {
      digit |= 32;
    }
    result.push(DIGITS[digit as usize] as char);
    if bits == 0 {
      break;
    }
  }
  result
}

pub(super) fn source_map_comment(file_name: &str, generated_prefix_lines: usize, mappings: &[FfiLineMapping]) -> String {
  let mut segments = vec![];
  let mut sources = vec![];
  let mut sources_content = vec![];
  for mapping in mappings {
    let source_index = sources.len();
    sources.push(mapping.source_name.clone());
    sources_content.push(mapping.source_content.clone());
    for (offset, line) in js_line_slices(&mapping.code).iter().enumerate() {
      let original_column_offset = if offset == 0 { mapping.original_first_column } else { 0 };
      for column in 0..=line.encode_utf16().count() {
        segments.push((
          generated_prefix_lines + mapping.generated_line + offset,
          column,
          source_index,
          mapping.original_line + offset,
          original_column_offset + column,
        ));
      }
    }
  }
  segments.sort_unstable();
  let mut encoded = String::new();
  let mut generated_line = 0;
  let mut previous_generated_column = 0_i64;
  let mut previous_source = 0_i64;
  let mut previous_original_line = 0_i64;
  let mut previous_original_column = 0_i64;
  for (line, column, source, original_line, original_column) in segments {
    while generated_line < line {
      encoded.push(';');
      generated_line += 1;
      previous_generated_column = 0;
    }
    if !encoded.is_empty() && !encoded.ends_with(';') {
      encoded.push(',');
    }
    encoded.push_str(&vlq(column as i64 - previous_generated_column));
    encoded.push_str(&vlq(source as i64 - previous_source));
    encoded.push_str(&vlq(original_line as i64 - previous_original_line));
    encoded.push_str(&vlq(original_column as i64 - previous_original_column));
    previous_generated_column = column as i64;
    previous_source = source as i64;
    previous_original_line = original_line as i64;
    previous_original_column = original_column as i64;
  }
  let map = json!({
    "version": 3,
    "file": file_name,
    "sources": sources,
    "sourcesContent": sources_content,
    "names": [],
    "mappings": encoded,
  });
  let data = base64::engine::general_purpose::STANDARD.encode(map.to_string());
  format!("\n//# sourceMappingURL=data:application/json;base64,{data}\n")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn maps_separated_source_ranges_at_line_granularity() {
    let comment = source_map_comment(
      "app.main.mjs",
      2,
      &[
        FfiLineMapping {
          generated_line: 3,
          original_line: 1,
          original_first_column: 0,
          code: "ab\nc".to_owned(),
          source_name: "calcit://app@0.1/app.main/a/inline".to_owned(),
          source_content: "\n(x) => {\n  throw x\n}".to_owned(),
        },
        FfiLineMapping {
          generated_line: 8,
          original_line: 0,
          original_first_column: 0,
          code: "x".to_owned(),
          source_name: "calcit://app@0.1/app.main/b/file/a.js".to_owned(),
          source_content: "(x) => x".to_owned(),
        },
      ],
    );
    let data = comment.trim().split_once(',').unwrap().1;
    let map: serde_json::Value = serde_json::from_slice(&base64::engine::general_purpose::STANDARD.decode(data).unwrap()).unwrap();
    assert!(map["mappings"].as_str().unwrap().starts_with(";;;;;AACA,CAAC,CAAC;AACF,CAAC"));
    assert_eq!(map["sources"].as_array().unwrap().len(), 2);
    assert_eq!(js_line_slices("α\r\nβ\rγ\u{2028}δ\u{2029}ε"), ["α", "β", "γ", "δ", "ε"]);
  }
}
