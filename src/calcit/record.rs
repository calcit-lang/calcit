use std::{cmp::Ordering, sync::Arc};

use cirru_edn::EdnTag;

use crate::Calcit;

use super::CalcitStruct;

#[derive(Debug, Clone)]
pub struct CalcitRecord {
  pub struct_ref: Arc<CalcitStruct>,
  pub values: Arc<Vec<Calcit>>,
  /// Trait implementations attached to this record (multiple allowed for composition)
  pub classes: Vec<Arc<CalcitRecord>>,
}

impl PartialEq for CalcitRecord {
  fn eq(&self, other: &Self) -> bool {
    self.struct_ref.name == other.struct_ref.name && self.struct_ref.fields == other.struct_ref.fields && self.values == other.values
  }
}

impl Eq for CalcitRecord {}

impl Default for CalcitRecord {
  fn default() -> CalcitRecord {
    CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(EdnTag::new("record"), vec![])),
      values: Arc::new(vec![]),
      classes: vec![],
    }
  }
}

impl CalcitRecord {
  pub fn name(&self) -> &EdnTag {
    &self.struct_ref.name
  }

  pub fn fields(&self) -> &Arc<Vec<EdnTag>> {
    &self.struct_ref.fields
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

  pub fn extend_field(&self, new_field: &EdnTag, new_tag: &Calcit, new_value: &Calcit) -> Result<CalcitRecord, String> {
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
            unreachable!("does not equal")
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
      _ => return Err("extend-field expected a record name, but received an invalid type".to_string()),
    };

    Ok(CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(new_name_id, next_fields)),
      values: Arc::new(next_values),
      classes: self.classes.clone(),
    })
  }
}
