use std::collections::HashMap;
use std::sync::Arc;

use cirru_edn::EdnTag;

use crate::calcit::{Calcit, CalcitGenericBound, CalcitImpl, CalcitList, CalcitStructDef, CalcitStructValue, CalcitTypeAnnotation};

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
pub struct CalcitEnumDef {
  name: EdnTag,
  generics: Arc<Vec<Arc<str>>>,
  where_bounds: Arc<Vec<CalcitGenericBound>>,
  variants: Arc<Vec<EnumVariant>>,
  /// Trait implementations attached to this enum (multiple allowed for composition)
  pub impls: Vec<Arc<CalcitImpl>>,
  /// Precomputed index for O(1) lookup by tag name; avoids linear scans on frequent queries.
  variant_index: Arc<HashMap<String, usize>>,
}

impl CalcitEnumDef {
  /// Create from a `CalcitStructValue` using the old-style enum definition format.
  /// The record's fields are variant tags and values are payload type lists.
  pub fn from_record(record: CalcitStructValue) -> Result<Self, String> {
    Self::from_arc(Arc::new(record))
  }

  pub fn from_arc(record: Arc<CalcitStructValue>) -> Result<Self, String> {
    let (variants, variant_index) = Self::collect_variants(&record)?;
    let name = record.name().to_owned();
    let generics = record.struct_ref.generics.clone();
    let where_bounds = record.struct_ref.where_bounds.clone();
    let impls = record.struct_ref.impls.clone();
    Ok(Self {
      name,
      generics,
      where_bounds,
      variants: Arc::new(variants),
      impls,
      variant_index: Arc::new(variant_index),
    })
  }

  pub fn name(&self) -> &EdnTag {
    &self.name
  }

  pub fn generics(&self) -> &[Arc<str>] {
    self.generics.as_ref()
  }

  pub fn where_bounds(&self) -> &[CalcitGenericBound] {
    self.where_bounds.as_ref()
  }

  /// Reconstruct a `CalcitStructValue` prototype from the enum's data.
  /// Used for serialization and backwards-compatibility paths that expect a record.
  pub fn to_record_prototype(&self) -> CalcitStructValue {
    let fields: Vec<EdnTag> = self.variants.iter().map(|v| v.tag.clone()).collect();
    let values: Vec<Calcit> = self
      .variants
      .iter()
      .map(|v| {
        if v.payload_types.is_empty() {
          Calcit::Nil
        } else {
          let items: Vec<Calcit> = v.payload_types.iter().map(|t| t.to_calcit()).collect();
          Calcit::List(Arc::new(CalcitList::from(items.as_slice())))
        }
      })
      .collect();
    let struct_def = CalcitStructDef {
      name: self.name.clone(),
      fields: Arc::new(fields),
      field_types: Arc::new(vec![crate::calcit::DYNAMIC_TYPE.clone(); values.len()]),
      generics: self.generics.clone(),
      where_bounds: self.where_bounds.clone(),
      impls: self.impls.clone(),
    };
    CalcitStructValue {
      struct_ref: Arc::new(struct_def),
      values: Arc::new(values),
    }
  }

  pub fn impls(&self) -> &[Arc<CalcitImpl>] {
    &self.impls
  }

  pub fn set_impls(&mut self, impls: Vec<Arc<CalcitImpl>>) {
    self.impls = impls;
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

  fn collect_variants(record: &CalcitStructValue) -> Result<(Vec<EnumVariant>, HashMap<String, usize>), String> {
    let mut variants: Vec<EnumVariant> = Vec::with_capacity(record.fields().len());
    let mut index: HashMap<String, usize> = HashMap::with_capacity(record.fields().len());
    let generics = record.struct_ref.generics.as_ref();

    for (idx, tag) in record.fields().iter().enumerate() {
      let payloads = Self::parse_payloads(
        record
          .values
          .get(idx)
          .ok_or_else(|| format!("enum `{}` is missing payload description for variant `{}`", record.name(), tag))?,
        tag,
        generics,
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

  fn parse_payloads(value: &Calcit, tag: &EdnTag, generics: &[Arc<str>]) -> Result<Vec<Arc<CalcitTypeAnnotation>>, String> {
    match value {
      Calcit::List(items) => {
        let mut payloads: Vec<Arc<CalcitTypeAnnotation>> = Vec::with_capacity(items.len());
        for item in items.iter() {
          let parsed = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(item, generics);
          parsed
            .validate_applied_type_args()
            .map_err(|e| format!("enum variant `{tag}` has invalid payload type annotation: {e}"))?;
          payloads.push(parsed);
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
  use crate::calcit::{CalcitEnumValue, CalcitList, CalcitStructDef, CalcitSymbolInfo, CalcitTypeAnnotation};

  fn symbol(name: &str) -> Calcit {
    Calcit::Symbol {
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("test.sum-type"),
        at_def: Arc::from("sum-type-tests"),
      }),
      location: None,
    }
  }

  fn empty_list() -> Calcit {
    Calcit::List(Arc::new(CalcitList::Vector(vec![])))
  }

  fn list_from(items: Vec<Calcit>) -> Calcit {
    Calcit::List(Arc::new(CalcitList::Vector(items)))
  }

  fn sample_enum_record() -> CalcitStructValue {
    CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(
        EdnTag::new("Result"),
        vec![EdnTag::new("err"), EdnTag::new("ok")],
      )),
      values: Arc::new(vec![list_from(vec![Calcit::tag("string")]), empty_list()]),
    }
  }

  #[test]
  fn parses_enum_prototype() {
    let record = sample_enum_record();
    let enum_proto = CalcitEnumDef::from_record(record).expect("valid enum");

    assert_eq!(enum_proto.name(), &EdnTag::new("Result"));
    let err_variant = enum_proto.find_variant_by_name("err").expect("err variant");
    assert_eq!(err_variant.arity(), 1);
    match err_variant.payload_types().first().map(|t| t.as_ref()) {
      Some(CalcitTypeAnnotation::String) => {}
      other => panic!("unexpected payload annotation: {other:?}"),
    }
    assert_eq!(enum_proto.find_variant_by_name("ok").unwrap().arity(), 0);
  }

  #[test]
  fn parses_generic_enum_prototype() {
    let record = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef {
        name: EdnTag::new("Result"),
        fields: Arc::new(vec![EdnTag::new("err"), EdnTag::new("ok")]),
        field_types: Arc::new(vec![crate::calcit::DYNAMIC_TYPE.clone(); 2]),
        generics: Arc::new(vec![Arc::from("T"), Arc::from("E")]),
        where_bounds: Arc::new(vec![]),
        impls: vec![],
      }),
      values: Arc::new(vec![list_from(vec![symbol("E")]), list_from(vec![symbol("T")])]),
    };

    let enum_proto = CalcitEnumDef::from_record(record).expect("valid generic enum");
    assert_eq!(enum_proto.generics(), &[Arc::from("T"), Arc::from("E")]);
    assert!(matches!(
      enum_proto.find_variant_by_name("ok").and_then(|v| v.payload_types().first()).map(|t| t.as_ref()),
      Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "T"
    ));
    assert!(matches!(
      enum_proto.find_variant_by_name("err").and_then(|v| v.payload_types().first()).map(|t| t.as_ref()),
      Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "E"
    ));
  }

  #[test]
  fn rejects_non_generic_struct_type_args_in_payloads() {
    let pair = CalcitStructDef::from_fields(EdnTag::new("Pair"), vec![EdnTag::new("left"), EdnTag::new("right")]);
    let applied_pair = Calcit::Enum(CalcitEnumValue {
      tag: Arc::new(Calcit::StructDef(pair)),
      extra: vec![Calcit::tag("number"), Calcit::tag("string")],
      sum_type: None,
    });
    let record = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(EdnTag::new("Wrapped"), vec![EdnTag::new("pair")])),
      values: Arc::new(vec![list_from(vec![applied_pair])]),
    };

    let err = CalcitEnumDef::from_record(record).expect_err("non-generic struct should reject type args in enum payloads");
    assert!(
      err.contains("enum variant `pair` has invalid payload type annotation")
        && err.contains("struct `Pair` is not generic but received 2 type argument(s)"),
      "unexpected error: {err}"
    );
  }
}
