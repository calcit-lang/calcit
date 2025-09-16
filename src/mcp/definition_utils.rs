use cirru_parser::Cirru;
use serde_json::Value;

/// Navigate to a specific coordinate in Cirru code (immutable)
pub fn navigate_to_coord<'a>(code: &'a Cirru, coord: &[usize]) -> Result<&'a Cirru, String> {
  let mut current = code;

  for (i, &index) in coord.iter().enumerate() {
    match current {
      Cirru::List(list) => {
        if index >= list.len() {
          return Err(format!(
            "Index {} out of bounds at coordinate position {} (list has {} elements)",
            index,
            i,
            list.len()
          ));
        }
        current = &list[index];
      }
      _ => {
        return Err(format!(
          "Cannot navigate into non-list at coordinate position {} (found: {})",
          i,
          match current {
            Cirru::Leaf(s) => format!("leaf '{s}'"),
            _ => "unknown".to_string(),
          }
        ));
      }
    }
  }

  Ok(current)
}

/// Navigate to a specific coordinate in Cirru code (mutable)
pub fn navigate_to_coord_mut<'a>(code: &'a mut Cirru, coord: &[usize]) -> Result<&'a mut Cirru, String> {
  let mut current = code;

  for (i, &index) in coord.iter().enumerate() {
    match current {
      Cirru::List(list) => {
        if index >= list.len() {
          return Err(format!(
            "Index {} out of bounds at coordinate position {} (list has {} elements)",
            index,
            i,
            list.len()
          ));
        }
        current = &mut list[index];
      }
      _ => {
        return Err(format!(
          "Cannot navigate into non-list at coordinate position {} (found: {})",
          i,
          match current {
            Cirru::Leaf(s) => format!("leaf '{s}'"),
            _ => "unknown".to_string(),
          }
        ));
      }
    }
  }

  Ok(current)
}

/// Validate that current Cirru structure matches expected pattern
pub fn validate_cirru_pattern(current: &Cirru, pattern: &Cirru, path: &str) -> Result<(), String> {
  match (current, pattern) {
    (Cirru::Leaf(current_val), Cirru::Leaf(pattern_val)) => {
      if current_val == pattern_val {
        Ok(())
      } else {
        Err(format!("Leaf mismatch at {path}: expected '{pattern_val}', found '{current_val}'"))
      }
    }
    (Cirru::List(current_list), Cirru::List(pattern_list)) => validate_list_pattern(current_list, pattern_list, path),
    (Cirru::Leaf(val), Cirru::List(_)) => Err(format!("Type mismatch at {path}: expected list, found leaf '{val}'")),
    (Cirru::List(_), Cirru::Leaf(val)) => Err(format!("Type mismatch at {path}: expected leaf '{val}', found list")),
  }
}

/// Validate that current list matches expected pattern list
pub fn validate_list_pattern(current_list: &[Cirru], pattern_list: &[Cirru], path: &str) -> Result<(), String> {
  if current_list.len() != pattern_list.len() {
    return Err(format!(
      "Length mismatch at {}: expected {} elements, found {}",
      path,
      pattern_list.len(),
      current_list.len()
    ));
  }

  for (i, (current_item, pattern_item)) in current_list.iter().zip(pattern_list.iter()).enumerate() {
    let item_path = if path.is_empty() {
      format!("[{i}]")
    } else {
      format!("{path}[{i}]")
    };
    validate_cirru_pattern(current_item, pattern_item, &item_path)?;
  }

  Ok(())
}

/// Parse coordinate array from JSON value
pub fn parse_coord_from_json(coord_value: &Value) -> Result<Vec<usize>, String> {
  match coord_value {
    Value::Array(arr) => {
      let mut coord = Vec::new();
      for (i, item) in arr.iter().enumerate() {
        match item {
          Value::Number(n) => {
            if let Some(int_val) = n.as_u64() {
              coord.push(int_val as usize);
            } else {
              return Err(format!("Coordinate element {i} is not a valid non-negative integer"));
            }
          }
          _ => return Err(format!("Coordinate element {i} is not a number")),
        }
      }
      Ok(coord)
    }
    _ => Err("Coordinate must be an array of non-negative integers".to_string()),
  }
}

/// Convert JSON value to Cirru structure
pub fn json_to_cirru_value(value: &Value) -> Result<Cirru, String> {
  match value {
    Value::String(s) => Ok(Cirru::Leaf(s.clone().into())),
    Value::Array(arr) => {
      let mut cirru_list = Vec::new();
      for item in arr {
        cirru_list.push(json_to_cirru_value(item)?);
      }
      Ok(Cirru::List(cirru_list))
    }
    Value::Number(n) => {
      if let Some(int_val) = n.as_i64() {
        Ok(Cirru::Leaf(int_val.to_string().into()))
      } else if let Some(float_val) = n.as_f64() {
        Ok(Cirru::Leaf(float_val.to_string().into()))
      } else {
        Err("Invalid number format".to_string())
      }
    }
    Value::Bool(b) => Ok(Cirru::Leaf(b.to_string().into())),
    Value::Null => Ok(Cirru::Leaf("nil".into())),
    _ => Err("Unsupported JSON value type for Cirru conversion".to_string()),
  }
}
