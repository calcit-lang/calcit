use cirru_edn::{Edn, EdnListView};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructuredOutputFormat {
  Human,
  Edn,
  Json,
}

impl StructuredOutputFormat {
  pub(crate) fn parse(raw: &str, command: &str) -> Result<Self, String> {
    match raw {
      "human" | "text" => Ok(Self::Human),
      "edn" => Ok(Self::Edn),
      "json" => Ok(Self::Json),
      _ => Err(format!("Unsupported {command} format '{raw}'. Expected human, edn, or json.")),
    }
  }

  /// Like [`parse`](Self::parse), but also accepts one legacy spelling of `human`
  /// (`text` for call-graph, `tree` for effects-graph) so older scripts keep working.
  pub(crate) fn parse_with_legacy_human(raw: &str, command: &str, legacy_human: &str) -> Result<Self, String> {
    if raw == legacy_human {
      Ok(Self::Human)
    } else {
      Self::parse(raw, command)
    }
  }

  /// Prints one JSON-shaped document as JSON (single line) or the equivalent Cirru EDN.
  pub(crate) fn print_value(self, value: &serde_json::Value) -> Result<(), String> {
    match self {
      Self::Json => println!("{value}"),
      Self::Edn => println!("{}", format_json_value_as_edn(value)?),
      Self::Human => return Err("Cannot print a human report as a structured document".to_owned()),
    }
    Ok(())
  }

  /// Same as [`print_value`](Self::print_value) for an already serialized JSON document.
  pub(crate) fn print_json_text(self, json: &str) -> Result<(), String> {
    match self {
      Self::Json => {
        println!("{json}");
        Ok(())
      }
      _ => {
        let value =
          serde_json::from_str(json).map_err(|error| format!("Failed to parse JSON envelope for Cirru EDN output: {error}"))?;
        self.print_value(&value)
      }
    }
  }
}

/// Resolves `--format` together with a legacy output-replacing `--json` switch.
/// `--json` alone selects JSON (with a deprecation warning); combining it with
/// `--format edn` is rejected because the two disagree.
pub(crate) fn resolve_format_with_legacy_json(raw: &str, legacy_json: bool, command: &str) -> Result<StructuredOutputFormat, String> {
  let format = StructuredOutputFormat::parse(raw, command)?;
  if !legacy_json {
    return Ok(format);
  }
  warn_deprecated_json_switch(command);
  match format {
    StructuredOutputFormat::Human | StructuredOutputFormat::Json => Ok(StructuredOutputFormat::Json),
    StructuredOutputFormat::Edn => Err(format!(
      "`{command}` got both `--json` and `--format edn`; drop the deprecated `--json`."
    )),
  }
}

/// Warns on stderr that a legacy `--json` switch is deprecated in favor of `--format json`.
pub(crate) fn warn_deprecated_json_switch(command: &str) {
  eprintln!(
    "[Deprecated] `calcit {command} --json` will be removed in a future non-patch release; use `--format json`. See docs/run/upgrade.md."
  );
}

fn json_key_to_edn(key: &str) -> Edn {
  Edn::tag(key.replace('_', "-"))
}

fn json_number_to_edn(value: &serde_json::Number) -> Result<Edn, String> {
  if let Some(number) = value.as_i64() {
    let converted = number as f64;
    return if converted >= i64::MIN as f64 && converted < i64::MAX as f64 && converted as i64 == number {
      Ok(Edn::Number(converted))
    } else {
      Err(format!("JSON integer cannot be represented exactly as Cirru EDN: {value}"))
    };
  }
  if let Some(number) = value.as_u64() {
    let converted = number as f64;
    return if converted < u64::MAX as f64 && converted as u64 == number {
      Ok(Edn::Number(converted))
    } else {
      Err(format!("JSON integer cannot be represented exactly as Cirru EDN: {value}"))
    };
  }
  value
    .as_f64()
    .map(Edn::Number)
    .ok_or_else(|| format!("JSON number cannot be represented as Cirru EDN: {value}"))
}

pub(crate) fn json_value_to_edn(value: &serde_json::Value) -> Result<Edn, String> {
  match value {
    serde_json::Value::Null => Ok(Edn::Nil),
    serde_json::Value::Bool(value) => Ok(Edn::Bool(*value)),
    serde_json::Value::Number(value) => json_number_to_edn(value),
    serde_json::Value::String(value) => Ok(Edn::str(value.as_str())),
    serde_json::Value::Array(values) => values
      .iter()
      .map(json_value_to_edn)
      .collect::<Result<Vec<_>, _>>()
      .map(|values| Edn::List(EdnListView(values))),
    serde_json::Value::Object(values) => {
      let mut normalized_keys = HashSet::new();
      let mut fields = Vec::with_capacity(values.len());
      for (key, value) in values {
        let normalized_key = key.replace('_', "-");
        if !normalized_keys.insert(normalized_key.clone()) {
          return Err(format!(
            "JSON object keys collide after Cirru EDN normalization: '{key}' becomes ':{normalized_key}'"
          ));
        }
        fields.push((json_key_to_edn(key), json_value_to_edn(value)?));
      }
      Ok(Edn::map_from_iter(fields))
    }
  }
}

pub(crate) fn format_json_value_as_edn(value: &serde_json::Value) -> Result<String, String> {
  let value = json_value_to_edn(value)?;
  cirru_edn::format(&value, true).map_err(|error| format!("Failed to render Cirru EDN output: {error}"))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn converts_json_envelope_keys_to_native_edn_tags() {
    let value = serde_json::json!({
      "schema_version": 1,
      "data": { "status": "passed", "checks": [] },
    });
    let rendered = format_json_value_as_edn(&value).expect("EDN should format");
    let parsed = cirru_edn::parse(&rendered).expect("EDN should parse");
    let Edn::Map(root) = parsed else {
      panic!("EDN envelope should be a map");
    };
    assert_eq!(root.get(&Edn::tag("schema-version")), Some(&Edn::Number(1.0)));
    let Some(Edn::Map(data)) = root.get(&Edn::tag("data")) else {
      panic!("EDN data should be a map");
    };
    assert_eq!(data.get(&Edn::tag("status")), Some(&Edn::str("passed")));
  }

  #[test]
  fn rejects_lossy_numbers_and_normalized_key_collisions() {
    assert!(json_value_to_edn(&serde_json::json!(9_007_199_254_740_993_u64)).is_err());
    assert!(json_value_to_edn(&serde_json::json!(i64::MAX)).is_err());
    assert!(json_value_to_edn(&serde_json::json!(u64::MAX)).is_err());
    assert!(json_value_to_edn(&serde_json::json!({ "same_key": 1, "same-key": 2 })).is_err());
  }

  #[test]
  fn legacy_human_spelling_is_accepted_alongside_unified_values() {
    for raw in ["text", "human"] {
      assert_eq!(
        StructuredOutputFormat::parse_with_legacy_human(raw, "call-graph", "text"),
        Ok(StructuredOutputFormat::Human)
      );
    }
    assert_eq!(
      StructuredOutputFormat::parse_with_legacy_human("tree", "effects-graph", "tree"),
      Ok(StructuredOutputFormat::Human)
    );
    assert_eq!(
      StructuredOutputFormat::parse_with_legacy_human("edn", "effects-graph", "tree"),
      Ok(StructuredOutputFormat::Edn)
    );
    assert!(StructuredOutputFormat::parse_with_legacy_human("yaml", "effects-graph", "tree").is_err());
  }

  #[test]
  fn legacy_json_switch_resolves_with_format() {
    use StructuredOutputFormat::*;
    assert_eq!(resolve_format_with_legacy_json("human", false, "query schema"), Ok(Human));
    assert_eq!(resolve_format_with_legacy_json("human", true, "query schema"), Ok(Json));
    assert_eq!(resolve_format_with_legacy_json("json", true, "query schema"), Ok(Json));
    assert_eq!(resolve_format_with_legacy_json("edn", false, "query schema"), Ok(Edn));
    assert!(resolve_format_with_legacy_json("edn", true, "query schema").is_err());
  }

  #[test]
  fn format_parser_lists_native_edn_before_json() {
    let error = StructuredOutputFormat::parse("yaml", "verify").expect_err("unknown format should fail");
    assert!(error.ends_with("Expected human, edn, or json."));
  }
}
