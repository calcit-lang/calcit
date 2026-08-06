use std::{cmp::Ordering, sync::Arc};

use cirru_edn::EdnTag;

use crate::Calcit;

use super::CalcitStructDef;

/// Display-only sentinel for anonymous structs (values without a `StructDef`).
/// Code must use `is_anonymous` rather than treating `_` as nominal identity.
pub const ANONYMOUS_STRUCT_NAME: &str = "_";

#[derive(Debug, Clone)]
pub struct CalcitStructValue {
  pub struct_ref: Arc<CalcitStructDef>,
  pub values: Arc<Vec<Calcit>>,
}

impl PartialEq for CalcitStructValue {
  fn eq(&self, other: &Self) -> bool {
    self.struct_ref.name == other.struct_ref.name && self.struct_ref.fields == other.struct_ref.fields && self.values == other.values
  }
}

impl Eq for CalcitStructValue {}

impl Default for CalcitStructValue {
  fn default() -> CalcitStructValue {
    CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(EdnTag::new("record"), vec![])),
      values: Arc::new(vec![]),
    }
  }
}

impl CalcitStructValue {
  pub fn name(&self) -> &EdnTag {
    &self.struct_ref.name
  }

  pub fn fields(&self) -> &Arc<Vec<EdnTag>> {
    &self.struct_ref.fields
  }

  /// Returns true when the value has no declared struct definition.
  pub fn is_anonymous(&self) -> bool {
    self.struct_ref.name.ref_str() == ANONYMOUS_STRUCT_NAME
  }

  /// Create an anonymous struct from sorted field-value pairs.
  /// Fields must already be sorted alphabetically.
  pub fn from_anonymous_pairs(fields: Vec<EdnTag>, values: Vec<Calcit>) -> Self {
    CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(EdnTag::new(ANONYMOUS_STRUCT_NAME), fields)),
      values: Arc::new(values),
    }
  }

  /// returns position of target
  pub fn index_of(&self, y: &str) -> Option<usize> {
    let xs: &[EdnTag] = &self.struct_ref.fields;
    if xs.is_empty() {
      return None;
    }
    let mut lower = 0;
    let mut upper = xs.len() - 1;

    while (upper - lower) > 1 {
      let pos = (lower + upper) >> 1;
      let v = xs.get(pos).unwrap();
      match y.cmp(v.ref_str()) {
        Ordering::Less => upper = pos - 1,
        Ordering::Greater => lower = pos + 1,
        Ordering::Equal => return Some(pos),
      }
    }

    match y {
      _ if y == xs[lower].ref_str() => Some(lower),
      _ if y == xs[upper].ref_str() => Some(upper),
      _ => None,
    }
  }

  pub fn get(&self, name: &str) -> Option<&Calcit> {
    match self.index_of(name) {
      Some(i) => Some(&self.values[i]),
      None => None,
    }
  }

  pub fn extend_field(&self, new_field: &EdnTag, new_tag: &Calcit, new_value: &Calcit) -> Result<CalcitStructValue, String> {
    let mut next_fields: Vec<EdnTag> = Vec::with_capacity(self.struct_ref.fields.len());
    let mut next_values: Vec<Calcit> = Vec::with_capacity(self.struct_ref.fields.len());
    let mut inserted: bool = false;

    for (i, k) in self.struct_ref.fields.iter().enumerate() {
      if inserted {
        next_fields.push(k.to_owned());
        next_values.push(self.values[i].to_owned());
      } else {
        match new_field.ref_str().cmp(k.ref_str()) {
          Ordering::Less => {
            next_fields.push(new_field.to_owned());
            next_values.push(new_value.to_owned());

            next_fields.push(k.to_owned());
            next_values.push(self.values[i].to_owned());
            inserted = true;
          }
          Ordering::Greater => {
            next_fields.push(k.to_owned());
            next_values.push(self.values[i].to_owned());
          }
          Ordering::Equal => {
            return Err(format!(
              "extend-field expected a new field, but `{}` already exists",
              new_field.ref_str()
            ));
          }
        }
      }
    }
    if !inserted {
      next_fields.push(new_field.to_owned());
      next_values.push(new_value.to_owned());
    }

    let new_name_id: EdnTag = match new_tag {
      Calcit::Str(s) | Calcit::Symbol { sym: s, .. } => EdnTag(s.to_owned()),
      Calcit::Tag(s) => s.to_owned(),
      _ => return Err("extend-field expected a struct name, but received an invalid type".to_string()),
    };

    Ok(CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(new_name_id, next_fields)),
      values: Arc::new(next_values),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extend_field_returns_err_on_duplicate_field() {
    let record = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(EdnTag::new("Person"), vec![EdnTag::new("age")])),
      values: Arc::new(vec![Calcit::Number(1.0)]),
    };

    let result = record.extend_field(&EdnTag::new("age"), &Calcit::tag("Person2"), &Calcit::Number(2.0));
    assert!(result.is_err());
  }
}
