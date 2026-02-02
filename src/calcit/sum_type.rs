use std::collections::HashMap;
use std::sync::Arc;

use cirru_edn::EdnTag;

use crate::calcit::{Calcit, CalcitRecord, CalcitTypeAnnotation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
  pub tag: EdnTag,
  pub payload_types: Arc<Vec<Arc<CalcitTypeAnnotation>>>,
}

impl EnumVariant {
  pub fn arity(&self) -> usize {
    self.payload_types.len()
  }

  pub fn payload_types(&self) -> &[Arc<CalcitTypeAnnotation>] {
    self.payload_types.as_ref()
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcitEnum {
  prototype: Arc<CalcitRecord>,
  variants: Arc<Vec<EnumVariant>>,
  class: Option<Arc<CalcitRecord>>,
  /// Precomputed index for O(1) lookup by tag name; avoids linear scans on frequent queries.
  variant_index: Arc<HashMap<String, usize>>,
}

impl CalcitEnum {
  pub fn from_record(record: CalcitRecord) -> Result<Self, String> {
    Self::from_arc(Arc::new(record))
  }

  pub fn from_arc(record: Arc<CalcitRecord>) -> Result<Self, String> {
    let (variants, variant_index) = Self::collect_variants(&record)?;
    let class = record.class.clone();
    Ok(Self {
      prototype: record,
      variants: Arc::new(variants),
      class,
      variant_index: Arc::new(variant_index),
    })
  }

  pub fn name(&self) -> &EdnTag {
    self.prototype.name()
  }

  pub fn prototype(&self) -> &CalcitRecord {
    &self.prototype
  }

  pub fn class(&self) -> Option<&Arc<CalcitRecord>> {
    self.class.as_ref()
  }

  pub fn set_class(&mut self, class: Option<Arc<CalcitRecord>>) {
    self.class = class;
  }

  pub fn variants(&self) -> &[EnumVariant] {
    &self.variants
  }

  pub fn find_variant(&self, tag: &EdnTag) -> Option<&EnumVariant> {
    self.variant_index.get(tag.ref_str()).map(|idx| &self.variants[*idx])
  }

  pub fn find_variant_by_name(&self, name: &str) -> Option<&EnumVariant> {
    self.variant_index.get(name).map(|idx| &self.variants[*idx])
  }

  fn collect_variants(record: &CalcitRecord) -> Result<(Vec<EnumVariant>, HashMap<String, usize>), String> {
    let mut variants: Vec<EnumVariant> = Vec::with_capacity(record.fields().len());
    let mut index: HashMap<String, usize> = HashMap::with_capacity(record.fields().len());

    for (idx, tag) in record.fields().iter().enumerate() {
      let payloads = Self::parse_payloads(
        record
          .values
          .get(idx)
          .ok_or_else(|| format!("enum `{}` is missing payload description for variant `{}`", record.name(), tag))?,
        tag,
      )?;

      let key = tag.ref_str().to_owned();
      if index.contains_key(&key) {
        return Err(format!("duplicated enum variant `{}` in `{}`", tag, record.name()));
      }

      let variant = EnumVariant {
        tag: tag.to_owned(),
        payload_types: Arc::new(payloads),
      };
      index.insert(key, variants.len());
      variants.push(variant);
    }

    Ok((variants, index))
  }

  fn parse_payloads(value: &Calcit, tag: &EdnTag) -> Result<Vec<Arc<CalcitTypeAnnotation>>, String> {
    match value {
      Calcit::List(items) => {
        let mut payloads: Vec<Arc<CalcitTypeAnnotation>> = Vec::with_capacity(items.len());
        for item in items.iter() {
          payloads.push(CalcitTypeAnnotation::parse_type_annotation_form(item));
        }
        Ok(payloads)
      }
      Calcit::Nil => Ok(vec![]),
      other => Err(format!(
        "enum variant `{tag}` expects a list of payload type hints, but received: {other}"
      )),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitList, CalcitStruct, CalcitTypeAnnotation};

  fn empty_list() -> Calcit {
    Calcit::List(Arc::new(CalcitList::Vector(vec![])))
  }

  fn list_from(items: Vec<Calcit>) -> Calcit {
    Calcit::List(Arc::new(CalcitList::Vector(items)))
  }

  fn sample_enum_record() -> CalcitRecord {
    CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::new("Result"),
        vec![EdnTag::new("err"), EdnTag::new("ok")],
      )),
      values: Arc::new(vec![list_from(vec![Calcit::tag("string")]), empty_list()]),
      class: None,
    }
  }

  #[test]
  fn parses_enum_prototype() {
    let record = sample_enum_record();
    let enum_proto = CalcitEnum::from_record(record).expect("valid enum");

    assert_eq!(enum_proto.name(), &EdnTag::new("Result"));
    let err_variant = enum_proto.find_variant_by_name("err").expect("err variant");
    assert_eq!(err_variant.arity(), 1);
    match err_variant.payload_types().first().map(|t| t.as_ref()) {
      Some(CalcitTypeAnnotation::String) => {}
      other => panic!("unexpected payload annotation: {other:?}"),
    }
    assert_eq!(enum_proto.find_variant_by_name("ok").unwrap().arity(), 0);
  }
}
