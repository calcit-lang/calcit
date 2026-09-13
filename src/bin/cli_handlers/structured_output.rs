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
  fn format_parser_lists_native_edn_before_json() {
    let error = StructuredOutputFormat::parse("yaml", "verify").expect_err("unknown format should fail");
    assert!(error.ends_with("Expected human, edn, or json."));
  }
}
