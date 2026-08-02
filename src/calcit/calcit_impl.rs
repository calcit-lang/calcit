use std::{cmp::Ordering, sync::Arc};

use cirru_edn::EdnTag;

use super::{Calcit, CalcitTrait};

#[derive(Debug, Clone)]
pub struct CalcitImpl {
  pub name: EdnTag,
  pub origin: Option<Arc<CalcitTrait>>,
  pub fields: Arc<Vec<EdnTag>>,
  pub values: Arc<Vec<Calcit>>,
}

impl CalcitImpl {
  pub fn from_record(record: &crate::calcit::CalcitRecord) -> Self {
    CalcitImpl {
      name: record.struct_ref.name.to_owned(),
      origin: None,
      fields: record.struct_ref.fields.to_owned(),
      values: record.values.to_owned(),
    }
  }

  pub fn name(&self) -> &EdnTag {
    &self.name
  }

  pub fn origin(&self) -> Option<&Arc<CalcitTrait>> {
    self.origin.as_ref()
  }

  pub fn trait_name(&self) -> Option<&EdnTag> {
    self.origin.as_ref().map(|trait_def| &trait_def.name)
  }

  pub fn is_inherent(&self) -> bool {
    self.origin.is_none()
  }

  pub fn implements_trait(&self, trait_def: &CalcitTrait) -> bool {
    self.origin.as_ref().is_some_and(|origin| origin.as_ref() == trait_def)
  }

  pub fn matches_trait_reference(&self, trait_def: &CalcitTrait) -> bool {
    self.origin.as_ref().is_some_and(|origin| origin.matches_reference(trait_def))
  }

  pub fn fields(&self) -> &Arc<Vec<EdnTag>> {
    &self.fields
  }

  pub fn get(&self, name: &str) -> Option<&Calcit> {
    match self.index_of(name) {
      Some(i) => Some(&self.values[i]),
      None => None,
    }
  }

  fn index_of(&self, y: &str) -> Option<usize> {
    let xs: &[EdnTag] = &self.fields;
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
}

impl PartialEq for CalcitImpl {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name && self.origin == other.origin && self.fields == other.fields && self.values == other.values
  }
}

impl Eq for CalcitImpl {}
