use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use cirru_edn::{Edn, EdnEnumView, EdnStructView};

use crate::builtins::quick_build_atom;
use crate::calcit::data_shape::{DataShapeGraph, DataShapeNode};
use crate::calcit::{self, Calcit, CalcitEnumValue, CalcitList, CalcitStructValue};

const MAX_DECODE_DEPTH: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EdnDecodeError {
  pub(crate) path: String,
  pub(crate) message: String,
}

impl EdnDecodeError {
  fn at(path: &str, message: impl Into<String>) -> Self {
    Self {
      path: path.to_owned(),
      message: message.into(),
    }
  }
}

impl fmt::Display for EdnDecodeError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "parse-cirru-edn-as failed at {}: {}", self.path, self.message)
  }
}

pub(crate) fn decode(shape: &DataShapeGraph, input: &Edn) -> Result<Calcit, EdnDecodeError> {
  let value = Decoder { shape }.decode_node(shape.root, input, "$", 0)?;
  if cfg!(debug_assertions) {
    shape
      .validate_value(&value)
      .map_err(|error| EdnDecodeError::at(&error.path, format!("internal data shape invariant failed: {}", error.message)))?;
  }
  Ok(value)
}

/// Decode an already-evaluated Calcit value at an application boundary. This
/// is deliberately distinct from Cirru EDN: named structs accept maps and
/// nominal `Option<T>` fields accept their raw payloads.
pub(crate) fn decode_map(shape: &DataShapeGraph, input: &Calcit) -> Result<Calcit, EdnDecodeError> {
  let value = MapDecoder { shape }.decode_node(shape.root, input, "$", 0)?;
  shape.validate_value(&value).map_err(|error| {
    EdnDecodeError::at(
      &error.path,
      format!("internal runtime-map decoder invariant failed: {}", error.message),
    )
  })?;
  Ok(value)
}

struct MapDecoder<'a> {
  shape: &'a DataShapeGraph,
}

impl MapDecoder<'_> {
  fn decode_node(&self, node_id: usize, input: &Calcit, path: &str, depth: usize) -> Result<Calcit, EdnDecodeError> {
    if depth > MAX_DECODE_DEPTH {
      return Err(EdnDecodeError::at(path, format!("decode nesting exceeds {MAX_DECODE_DEPTH}")));
    }
    let node = self
      .shape
      .nodes
      .get(node_id)
      .ok_or_else(|| EdnDecodeError::at(path, format!("invalid data shape node #{node_id}")))?;
    match node {
      DataShapeNode::Dynamic => Ok(input.to_owned()),
      DataShapeNode::Unit => {
        if matches!(input, Calcit::Nil) {
          Ok(Calcit::Nil)
        } else {
          Err(map_kind_mismatch(path, "nil", input))
        }
      }
      DataShapeNode::Bool => {
        if matches!(input, Calcit::Bool(_)) {
          Ok(input.to_owned())
        } else {
          Err(map_kind_mismatch(path, "bool", input))
        }
      }
      DataShapeNode::Number => {
        if matches!(input, Calcit::Number(_)) {
          Ok(input.to_owned())
        } else {
          Err(map_kind_mismatch(path, "number", input))
        }
      }
      DataShapeNode::String => {
        if matches!(input, Calcit::Str(_)) {
          Ok(input.to_owned())
        } else {
          Err(map_kind_mismatch(path, "string", input))
        }
      }
      DataShapeNode::Symbol => {
        if matches!(input, Calcit::Symbol { .. }) {
          Ok(input.to_owned())
        } else {
          Err(map_kind_mismatch(path, "symbol", input))
        }
      }
      DataShapeNode::Tag => {
        if matches!(input, Calcit::Tag(_)) {
          Ok(input.to_owned())
        } else {
          Err(map_kind_mismatch(path, "tag", input))
        }
      }
      DataShapeNode::Buffer => {
        if matches!(input, Calcit::Buffer(_)) {
          Ok(input.to_owned())
        } else {
          Err(map_kind_mismatch(path, "buffer", input))
        }
      }
      DataShapeNode::CirruQuote => {
        if matches!(input, Calcit::CirruQuote(_)) {
          Ok(input.to_owned())
        } else {
          Err(map_kind_mismatch(path, "cirru-quote", input))
        }
      }
      DataShapeNode::Optional(inner) => {
        if matches!(input, Calcit::Nil) {
          Ok(Calcit::Nil)
        } else {
          self.decode_node(*inner, input, path, depth + 1)
        }
      }
      DataShapeNode::MapOption { nominal, inner, .. } => {
        if let Calcit::Enum(enum_value) = input
          && enum_value.sum_type.as_ref().is_some_and(|actual| actual.name() == nominal.name())
        {
          return match (enum_value.tag.as_ref(), enum_value.extra.as_slice()) {
            (Calcit::Tag(tag), []) if tag.ref_str() == "none" => Ok(input.to_owned()),
            (Calcit::Tag(tag), [item]) if tag.ref_str() == "some" => Ok(Calcit::Enum(CalcitEnumValue {
              tag: enum_value.tag.clone(),
              extra: vec![self.decode_node(*inner, item, path, depth + 1)?],
              sum_type: Some(nominal.clone()),
            })),
            _ => Err(EdnDecodeError::at(path, "invalid Option value")),
          };
        }
        let payload = self.decode_node(*inner, input, path, depth + 1)?;
        Ok(Calcit::Enum(CalcitEnumValue {
          tag: Arc::new(Calcit::Tag(cirru_edn::EdnTag::new("some"))),
          extra: vec![payload],
          sum_type: Some(nominal.clone()),
        }))
      }
      DataShapeNode::List(inner) => {
        let Calcit::List(items) = input else {
          return Err(map_kind_mismatch(path, "list", input));
        };
        let mut values = Vec::with_capacity(items.len());
        for (idx, item) in items.iter().enumerate() {
          values.push(self.decode_node(*inner, item, &format!("{path}[{idx}]"), depth + 1)?);
        }
        Ok(Calcit::List(Arc::new(CalcitList::Vector(values))))
      }
      DataShapeNode::Set(inner) => {
        let Calcit::Set(items) = input else {
          return Err(map_kind_mismatch(path, "set", input));
        };
        let mut values = rpds::HashTrieSet::new_sync();
        for item in items {
          let decoded = self.decode_node(*inner, item, &format!("{path}.item"), depth + 1)?;
          if values.contains(&decoded) {
            return Err(EdnDecodeError::at(path, "duplicate decoded set value"));
          }
          values.insert_mut(decoded);
        }
        Ok(Calcit::Set(values))
      }
      DataShapeNode::Map { key, value } => {
        let Calcit::Map(items) = input else {
          return Err(map_kind_mismatch(path, "map", input));
        };
        let mut values = rpds::HashTrieMap::new_sync();
        for (raw_key, raw_value) in items {
          let decoded_key = self.decode_node(*key, raw_key, &format!("{path}.key"), depth + 1)?;
          let decoded_value = self.decode_node(*value, raw_value, &format!("{path}.value"), depth + 1)?;
          if values.contains_key(&decoded_key) {
            return Err(EdnDecodeError::at(path, "duplicate decoded map key"));
          }
          values.insert_mut(decoded_key, decoded_value);
        }
        Ok(Calcit::Map(values))
      }
      DataShapeNode::Struct { nominal, fields, .. } => {
        let Calcit::Map(items) = input else {
          return Err(map_kind_mismatch(path, &format!("map for struct :{}", nominal.name), input));
        };
        let mut seen_fields = HashSet::new();
        for key in items.keys() {
          let name = match key {
            Calcit::Tag(tag) => tag.ref_str(),
            Calcit::Str(text) => text,
            _ => return Err(EdnDecodeError::at(path, "struct map keys must be tags or strings")),
          };
          if !fields.iter().any(|(field, _)| field.ref_str() == name) {
            return Err(EdnDecodeError::at(
              path,
              format!("struct :{} has unknown field :{name}", nominal.name),
            ));
          }
          if !seen_fields.insert(name) {
            return Err(EdnDecodeError::at(
              path,
              format!("struct :{} has duplicate field :{name}", nominal.name),
            ));
          }
        }
        let mut values = Vec::with_capacity(fields.len());
        for (field, child) in fields {
          let raw = items
            .get(&Calcit::Tag(field.clone()))
            .or_else(|| items.get(&Calcit::Str(Arc::from(field.ref_str()))));
          match (raw, self.shape.nodes.get(*child)) {
            (Some(value), _) => values.push(self.decode_node(*child, value, &format!("{path}.{}", field.ref_str()), depth + 1)?),
            (None, Some(DataShapeNode::MapOption { nominal, .. })) => values.push(Calcit::Enum(CalcitEnumValue {
              tag: Arc::new(Calcit::Tag(cirru_edn::EdnTag::new("none"))),
              extra: vec![],
              sum_type: Some(nominal.clone()),
            })),
            (None, _) => {
              return Err(EdnDecodeError::at(
                path,
                format!("struct :{} is missing required field :{}", nominal.name, field.ref_str()),
              ));
            }
          }
        }
        Ok(Calcit::Struct(CalcitStructValue {
          struct_ref: nominal.clone(),
          values: Arc::new(values),
        }))
      }
      DataShapeNode::Enum { nominal, variants, .. } => {
        let Calcit::Enum(enum_value) = input else {
          return Err(map_kind_mismatch(path, &format!("enum :{}", nominal.name()), input));
        };
        let Some(actual_nominal) = enum_value.sum_type.as_ref() else {
          return Err(EdnDecodeError::at(path, format!("expected nominal enum :{}", nominal.name())));
        };
        if !Arc::ptr_eq(actual_nominal, nominal) {
          return Err(EdnDecodeError::at(
            path,
            format!("expected enum :{}, got :{}", nominal.name(), actual_nominal.name()),
          ));
        }
        let Calcit::Tag(tag) = enum_value.tag.as_ref() else {
          return Err(EdnDecodeError::at(path, "enum variant is not a tag"));
        };
        let Some((_, payload_nodes)) = variants.iter().find(|(candidate, _)| candidate == tag) else {
          return Err(EdnDecodeError::at(path, format!("enum :{} has no variant :{tag}", nominal.name())));
        };
        if enum_value.extra.len() != payload_nodes.len() {
          return Err(EdnDecodeError::at(
            path,
            format!(
              "enum :{} variant :{tag} expects {} payload(s), got {}",
              nominal.name(),
              payload_nodes.len(),
              enum_value.extra.len()
            ),
          ));
        }
        let mut values = Vec::with_capacity(payload_nodes.len());
        for (idx, (payload_node, item)) in payload_nodes.iter().zip(enum_value.extra.iter()).enumerate() {
          values.push(self.decode_node(*payload_node, item, &format!("{path}.payload[{idx}]"), depth + 1)?);
        }
        Ok(Calcit::Enum(CalcitEnumValue {
          tag: enum_value.tag.clone(),
          extra: values,
          sum_type: Some(nominal.clone()),
        }))
      }
      DataShapeNode::Ref(_) => {
        self
          .shape
          .validate_node_value(node_id, input, path, depth)
          .map_err(|error| EdnDecodeError::at(&error.path, error.message))?;
        Ok(input.to_owned())
      }
    }
  }
}

fn map_kind_mismatch(path: &str, expected: &str, actual: &Calcit) -> EdnDecodeError {
  EdnDecodeError::at(
    path,
    format!("expected {expected}, got {}", crate::calcit::brief_type_of_value(actual)),
  )
}

struct Decoder<'a> {
  shape: &'a DataShapeGraph,
}

impl Decoder<'_> {
  fn decode_node(&self, node_id: usize, input: &Edn, path: &str, depth: usize) -> Result<Calcit, EdnDecodeError> {
    if depth > MAX_DECODE_DEPTH {
      return Err(EdnDecodeError::at(path, format!("decode nesting exceeds {MAX_DECODE_DEPTH}")));
    }
    let Some(node) = self.shape.nodes.get(node_id) else {
      return Err(EdnDecodeError::at(path, format!("invalid data shape node #{node_id}")));
    };

    match node {
      DataShapeNode::Dynamic | DataShapeNode::MapOption { .. } => Err(EdnDecodeError::at(
        path,
        "internal decoder error: open runtime-map node is unavailable for Cirru EDN",
      )),
      DataShapeNode::Unit => match input {
        Edn::Nil => Ok(Calcit::Nil),
        _ => Err(kind_mismatch(path, "nil", input)),
      },
      DataShapeNode::Bool => match input {
        Edn::Bool(value) => Ok(Calcit::Bool(*value)),
        _ => Err(kind_mismatch(path, "bool", input)),
      },
      DataShapeNode::Number => match input {
        Edn::Number(value) => Ok(Calcit::Number(*value)),
        _ => Err(kind_mismatch(path, "number", input)),
      },
      DataShapeNode::String => match input {
        Edn::Str(value) => Ok(Calcit::Str(value.clone())),
        _ => Err(kind_mismatch(path, "string", input)),
      },
      DataShapeNode::Symbol => match input {
        Edn::Symbol(value) => Ok(Calcit::Symbol {
          sym: value.clone(),
          info: Arc::new(crate::calcit::CalcitSymbolInfo {
            at_ns: calcit::GEN_NS.into(),
            at_def: calcit::GENERATED_DEF.into(),
          }),
          location: None,
        }),
        _ => Err(kind_mismatch(path, "symbol", input)),
      },
      DataShapeNode::Tag => match input {
        Edn::Tag(value) => Ok(Calcit::Tag(value.clone())),
        _ => Err(kind_mismatch(path, "tag", input)),
      },
      DataShapeNode::Buffer => match input {
        Edn::Buffer(value) => Ok(Calcit::Buffer(value.clone())),
        _ => Err(kind_mismatch(path, "buffer", input)),
      },
      DataShapeNode::CirruQuote => match input {
        Edn::Quote(value) => Ok(Calcit::CirruQuote(value.clone())),
        _ => Err(kind_mismatch(path, "cirru-quote", input)),
      },
      DataShapeNode::Optional(inner) => {
        if matches!(input, Edn::Nil) {
          Ok(Calcit::Nil)
        } else {
          self.decode_node(*inner, input, path, depth + 1)
        }
      }
      DataShapeNode::List(inner) => match input {
        Edn::List(items) => {
          let mut values = Vec::with_capacity(items.0.len());
          for (idx, item) in items.0.iter().enumerate() {
            values.push(self.decode_node(*inner, item, &format!("{path}[{idx}]"), depth + 1)?);
          }
          Ok(Calcit::List(Arc::new(CalcitList::Vector(values))))
        }
        _ => Err(kind_mismatch(path, "list", input)),
      },
      DataShapeNode::Set(inner) => match input {
        Edn::Set(items) => {
          let mut values = rpds::HashTrieSet::new_sync();
          for item in items.0.iter() {
            let item_path = format!("{path}.item");
            let decoded = self.decode_node(*inner, item, &item_path, depth + 1)?;
            if values.contains(&decoded) {
              return Err(EdnDecodeError::at(&item_path, "duplicate decoded set value"));
            }
            values.insert_mut(decoded);
          }
          Ok(Calcit::Set(values))
        }
        _ => Err(kind_mismatch(path, "set", input)),
      },
      DataShapeNode::Map { key, value } => match input {
        Edn::Map(entries) => {
          let mut values = rpds::HashTrieMap::new_sync();
          for (raw_key, raw_value) in entries.0.iter() {
            let key_path = format!("{path}.key");
            let decoded_key = self.decode_node(*key, raw_key, &key_path, depth + 1)?;
            if values.contains_key(&decoded_key) {
              return Err(EdnDecodeError::at(&key_path, "duplicate decoded map key"));
            }
            let decoded_value = self.decode_node(*value, raw_value, &format!("{path}.value"), depth + 1)?;
            values.insert_mut(decoded_key, decoded_value);
          }
          Ok(Calcit::Map(values))
        }
        _ => Err(kind_mismatch(path, "map", input)),
      },
      DataShapeNode::Ref(inner) => match input {
        Edn::Atom(value) => {
          let decoded = self.decode_node(*inner, value, &format!("{path}.value"), depth + 1)?;
          Ok(quick_build_atom(decoded))
        }
        _ => Err(kind_mismatch(path, "atom", input)),
      },
      DataShapeNode::Struct { nominal, fields, .. } => match input {
        Edn::Struct(EdnStructView { name, pairs }) => {
          if name.as_ref() != nominal.name.ref_str() {
            return Err(EdnDecodeError::at(
              path,
              format!("expected struct {}, got struct {name}", nominal.name),
            ));
          }

          let expected_names: HashSet<&str> = fields.iter().map(|(field, _)| field.ref_str()).collect();
          let actual_names: HashSet<&str> = pairs.iter().map(|(field, _)| field.ref_str()).collect();
          if actual_names.len() != pairs.len() {
            let duplicates = sorted_duplicate_names(pairs.iter().map(|(field, _)| field.ref_str()));
            return Err(EdnDecodeError::at(
              path,
              format!("struct :{} has duplicate fields [{}]", nominal.name, duplicates.join(", ")),
            ));
          }
          if expected_names != actual_names {
            let missing = sorted_name_diff(&expected_names, &actual_names);
            let unknown = sorted_name_diff(&actual_names, &expected_names);
            return Err(EdnDecodeError::at(
              path,
              format!(
                "struct :{} fields mismatch; missing [{}], unknown [{}]",
                nominal.name,
                missing.join(", "),
                unknown.join(", ")
              ),
            ));
          }

          let mut values = Vec::with_capacity(fields.len());
          for (field, field_node) in fields {
            let (_, raw_value) = pairs
              .iter()
              .find(|(candidate, _)| candidate == field)
              .expect("struct field sets were checked");
            values.push(self.decode_node(*field_node, raw_value, &format!("{path}.{}", field.ref_str()), depth + 1)?);
          }
          Ok(Calcit::Struct(CalcitStructValue {
            struct_ref: nominal.clone(),
            values: Arc::new(values),
          }))
        }
        _ => Err(kind_mismatch(path, &format!("struct :{}", nominal.name), input)),
      },
      DataShapeNode::Enum { nominal, variants, .. } => match input {
        Edn::Enum(EdnEnumView { variant, type_name, extra }) => {
          let Some(actual_enum_name) = type_name.as_deref() else {
            return Err(EdnDecodeError::at(
              path,
              format!("expected enum :{}, got anonymous enum", nominal.name()),
            ));
          };
          if actual_enum_name != nominal.name().ref_str() {
            return Err(EdnDecodeError::at(
              path,
              format!("expected enum :{}, got enum :{actual_enum_name}", nominal.name()),
            ));
          }
          let actual_tag = variant.as_ref();
          let Some((variant_tag, payload_nodes)) = variants.iter().find(|(candidate, _)| candidate.ref_str() == actual_tag) else {
            return Err(EdnDecodeError::at(
              path,
              format!("enum :{} has no variant :{actual_tag}", nominal.name()),
            ));
          };
          if extra.len() != payload_nodes.len() {
            return Err(EdnDecodeError::at(
              path,
              format!(
                "enum :{} variant :{} expects {} payload(s), got {}",
                nominal.name(),
                variant_tag,
                payload_nodes.len(),
                extra.len()
              ),
            ));
          }
          let mut values = Vec::with_capacity(payload_nodes.len());
          for (idx, (payload_node, raw_value)) in payload_nodes.iter().zip(extra.iter()).enumerate() {
            values.push(self.decode_node(*payload_node, raw_value, &format!("{path}.payload[{idx}]"), depth + 1)?);
          }
          Ok(Calcit::Enum(CalcitEnumValue {
            tag: Arc::new(Calcit::Tag(variant_tag.clone())),
            extra: values,
            sum_type: Some(nominal.clone()),
          }))
        }
        _ => Err(kind_mismatch(path, &format!("enum :{}", nominal.name()), input)),
      },
    }
  }
}

fn edn_kind(value: &Edn) -> &'static str {
  match value {
    Edn::Nil => "nil",
    Edn::Bool(_) => "bool",
    Edn::Number(_) => "number",
    Edn::Symbol(_) => "symbol",
    Edn::Tag(_) => "tag",
    Edn::Str(_) => "string",
    Edn::Quote(_) => "cirru-quote",
    Edn::Enum(enum_value) if enum_value.type_name.is_some() => "enum",
    Edn::Enum(_) => "anonymous-enum",
    Edn::List(_) => "list",
    Edn::Set(_) => "set",
    Edn::Map(_) => "map",
    Edn::Struct(_) => "struct",
    Edn::Buffer(_) => "buffer",
    Edn::AnyRef(_) => "any-ref",
    Edn::Atom(_) => "atom",
  }
}

fn kind_mismatch(path: &str, expected: &str, actual: &Edn) -> EdnDecodeError {
  EdnDecodeError::at(path, format!("expected {expected}, got {}", edn_kind(actual)))
}

fn sorted_name_diff<'a>(left: &HashSet<&'a str>, right: &HashSet<&'a str>) -> Vec<&'a str> {
  let mut values: Vec<&str> = left.difference(right).copied().collect();
  values.sort_unstable();
  values
}

fn sorted_duplicate_names<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
  let mut seen = HashSet::new();
  let mut duplicates = HashSet::new();
  for value in values {
    if !seen.insert(value) {
      duplicates.insert(value);
    }
  }
  let mut values: Vec<&str> = duplicates.into_iter().collect();
  values.sort_unstable();
  values
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::data_shape::DataShapeGraph;
  use crate::calcit::{CalcitEnumDef, CalcitStructDef, CalcitTypeAnnotation, EnumVariant};
  use cirru_edn::EdnTag;

  fn person_struct() -> Arc<CalcitStructDef> {
    Arc::new(CalcitStructDef {
      name: EdnTag::new("Person"),
      fields: Arc::new(vec![EdnTag::new("age"), EdnTag::new("name")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    })
  }

  fn option_enum() -> Arc<CalcitEnumDef> {
    let prototype = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef {
        name: EdnTag::new("Option"),
        fields: Arc::new(vec![EdnTag::new("none"), EdnTag::new("some")]),
        field_types: Arc::new(vec![
          Arc::new(CalcitTypeAnnotation::Dynamic),
          Arc::new(CalcitTypeAnnotation::Dynamic),
        ]),
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        impls: vec![],
      }),
      values: Arc::new(vec![
        Calcit::List(Arc::new(CalcitList::default())),
        Calcit::List(Arc::new(CalcitList::Vector(vec![Calcit::Number(0.0)]))),
      ]),
    };
    Arc::new(CalcitEnumDef::from_struct(prototype).expect("option enum"))
  }

  #[test]
  fn runtime_map_decode_requires_fields_lifts_option_and_rejects_unknown_keys() {
    let response = Arc::new(CalcitStructDef {
      name: EdnTag::new("Response"),
      fields: Arc::new(vec![EdnTag::new("code"), EdnTag::new("message"), EdnTag::new("body")]),
      field_types: Arc::new(vec![
        Arc::new(CalcitTypeAnnotation::Number),
        Arc::new(CalcitTypeAnnotation::Dynamic),
        Arc::new(CalcitTypeAnnotation::Dynamic),
      ]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let option = option_enum();
    let shape = DataShapeGraph::from_nodes(
      0,
      vec![
        DataShapeNode::Struct {
          nominal: response.clone(),
          nominal_path: None,
          type_args: Arc::new(vec![]),
          fields: vec![(EdnTag::new("code"), 1), (EdnTag::new("message"), 2), (EdnTag::new("body"), 4)],
        },
        DataShapeNode::Number,
        DataShapeNode::MapOption {
          nominal: option.clone(),
          nominal_path: None,
          inner: 3,
        },
        DataShapeNode::String,
        DataShapeNode::Dynamic,
      ],
    )
    .expect("response shape");
    let raw = Calcit::Map(
      rpds::HashTrieMap::new_sync()
        .insert(Calcit::Tag(EdnTag::new("code")), Calcit::Number(200.0))
        .insert(Calcit::Tag(EdnTag::new("message")), Calcit::Str(Arc::from("ok")))
        .insert(Calcit::Tag(EdnTag::new("body")), Calcit::Map(Default::default())),
    );
    let decoded = decode_map(&shape, &raw).expect("decode response");
    let Calcit::Struct(response_value) = decoded else {
      panic!("expected response struct")
    };
    assert_eq!(response_value.values[0], Calcit::Number(200.0));
    assert!(
      matches!(&response_value.values[1], Calcit::Enum(value) if value.tag.as_ref() == &Calcit::Tag(EdnTag::new("some")) && value.sum_type.as_ref().is_some_and(|actual| Arc::ptr_eq(actual, &option)))
    );
    let no_message = Calcit::Map(
      rpds::HashTrieMap::new_sync()
        .insert(Calcit::Tag(EdnTag::new("code")), Calcit::Number(204.0))
        .insert(Calcit::Tag(EdnTag::new("body")), Calcit::Map(Default::default())),
    );
    let Calcit::Struct(no_message_value) = decode_map(&shape, &no_message).expect("decode missing Option") else {
      panic!("expected response struct")
    };
    assert!(
      matches!(&no_message_value.values[1], Calcit::Enum(value) if value.tag.as_ref() == &Calcit::Tag(EdnTag::new("none")) && value.extra.is_empty() && value.sum_type.as_ref().is_some_and(|actual| Arc::ptr_eq(actual, &option)))
    );
    let prewrapped = Calcit::Map(
      rpds::HashTrieMap::new_sync()
        .insert(Calcit::Tag(EdnTag::new("code")), Calcit::Number(201.0))
        .insert(
          Calcit::Tag(EdnTag::new("message")),
          Calcit::Enum(CalcitEnumValue {
            tag: Arc::new(Calcit::Tag(EdnTag::new("some"))),
            extra: vec![Calcit::Str(Arc::from("already"))],
            sum_type: Some(option.clone()),
          }),
        )
        .insert(Calcit::Tag(EdnTag::new("body")), Calcit::Map(Default::default())),
    );
    let Calcit::Struct(prewrapped_value) = decode_map(&shape, &prewrapped).expect("decode Option") else {
      panic!("expected response struct")
    };
    assert!(
      matches!(&prewrapped_value.values[1], Calcit::Enum(value) if value.tag.as_ref() == &Calcit::Tag(EdnTag::new("some")) && value.extra == [Calcit::Str(Arc::from("already"))] && value.sum_type.as_ref().is_some_and(|actual| Arc::ptr_eq(actual, &option)))
    );
    let missing = Calcit::Map(rpds::HashTrieMap::new_sync());
    assert!(
      decode_map(&shape, &missing)
        .expect_err("missing required code")
        .message
        .contains("missing required field :code")
    );
    let unknown = Calcit::Map(
      rpds::HashTrieMap::new_sync()
        .insert(Calcit::Tag(EdnTag::new("code")), Calcit::Number(200.0))
        .insert(Calcit::Tag(EdnTag::new("extra")), Calcit::Bool(true)),
    );
    assert!(
      decode_map(&shape, &unknown)
        .expect_err("unknown key")
        .message
        .contains("unknown field :extra")
    );
  }

  #[test]
  fn decodes_struct_fields_deeply_and_preserves_nominal_identity() {
    let person = person_struct();
    let shape =
      DataShapeGraph::build(&CalcitTypeAnnotation::Struct(person.clone(), Arc::new(vec![])), "tests.edn").expect("derive person shape");
    let input = cirru_edn::parse("%{} :Person (:age 23) (:name |Ada)").expect("parse edn");
    let decoded = decode(&shape, &input).expect("decode person");
    let Calcit::Struct(struct_value) = decoded else {
      panic!("expected struct");
    };
    assert!(Arc::ptr_eq(&struct_value.struct_ref, &person));
    assert_eq!(struct_value.values.as_ref(), &[Calcit::Number(23.0), Calcit::Str(Arc::from("Ada"))]);
  }

  #[test]
  fn reports_nested_field_path() {
    let person = person_struct();
    let target = CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Struct(person, Arc::new(vec![]))));
    let shape = DataShapeGraph::build(&target, "tests.edn").expect("derive list shape");
    let input = cirru_edn::parse("[] $ %{} :Person (:age |old) (:name |Ada)").expect("parse edn");
    let error = decode(&shape, &input).expect_err("age must fail");
    assert_eq!(error.path, "$[0].age");
    assert!(error.message.contains("expected number, got string"));
  }

  #[test]
  fn rejects_duplicate_struct_fields() {
    let person = person_struct();
    let shape =
      DataShapeGraph::build(&CalcitTypeAnnotation::Struct(person, Arc::new(vec![])), "tests.edn").expect("derive person shape");
    let input = cirru_edn::parse("%{} :Person (:age 23) (:age 24) (:name |Ada)").expect("parse duplicate fields");
    let error = decode(&shape, &input).expect_err("duplicate fields must fail");
    assert_eq!(error.path, "$");
    assert!(error.message.contains("duplicate fields [age]"), "unexpected error: {error}");
  }

  #[test]
  fn rejects_duplicate_decoded_set_values() {
    let target = CalcitTypeAnnotation::Set(Arc::new(CalcitTypeAnnotation::Struct(person_struct(), Arc::new(vec![]))));
    let shape = DataShapeGraph::build(&target, "tests.edn").expect("derive person set shape");
    let input = cirru_edn::parse("#{} (%{} :Person (:age 23) (:name |Ada)) (%{} :Person (:name |Ada) (:age 23))")
      .expect("parse colliding set values");
    let error = decode(&shape, &input).expect_err("decoded set values must remain unique");
    assert_eq!(error.path, "$.item");
    assert_eq!(error.message, "duplicate decoded set value");
  }

  #[test]
  fn rejects_duplicate_decoded_map_keys() {
    let key = Arc::new(CalcitTypeAnnotation::Struct(person_struct(), Arc::new(vec![])));
    let target = CalcitTypeAnnotation::Map(key, Arc::new(CalcitTypeAnnotation::String));
    let shape = DataShapeGraph::build(&target, "tests.edn").expect("derive person map shape");
    let input = cirru_edn::parse("{} ((%{} :Person (:age 23) (:name |Ada)) |first) ((%{} :Person (:name |Ada) (:age 23)) |second)")
      .expect("parse colliding map keys");
    let error = decode(&shape, &input).expect_err("decoded map keys must remain unique");
    assert_eq!(error.path, "$.key");
    assert_eq!(error.message, "duplicate decoded map key");
  }

  #[test]
  fn decodes_unique_set_values_and_map_keys() {
    let person = Arc::new(CalcitTypeAnnotation::Struct(person_struct(), Arc::new(vec![])));
    let set_shape = DataShapeGraph::build(&CalcitTypeAnnotation::Set(person.clone()), "tests.edn").expect("derive person set shape");
    let set_input = cirru_edn::parse("#{} (%{} :Person (:age 23) (:name |Ada)) (%{} :Person (:age 24) (:name |Bob))")
      .expect("parse unique set values");
    let Calcit::Set(set) = decode(&set_shape, &set_input).expect("decode unique set values") else {
      panic!("expected set");
    };
    assert_eq!(set.size(), 2);

    let map_shape = DataShapeGraph::build(
      &CalcitTypeAnnotation::Map(person, Arc::new(CalcitTypeAnnotation::String)),
      "tests.edn",
    )
    .expect("derive person map shape");
    let map_input = cirru_edn::parse("{} ((%{} :Person (:age 23) (:name |Ada)) |first) ((%{} :Person (:age 24) (:name |Bob)) |second)")
      .expect("parse unique map keys");
    let Calcit::Map(map) = decode(&map_shape, &map_input).expect("decode unique map keys") else {
      panic!("expected map");
    };
    assert_eq!(map.size(), 2);
  }

  #[test]
  fn decodes_enum_and_checks_payload_arity() {
    let prototype = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef {
        name: EdnTag::new("ResultText"),
        fields: Arc::new(vec![EdnTag::new("err"), EdnTag::new("ok")]),
        field_types: Arc::new(vec![calcit::DYNAMIC_TYPE.clone(), calcit::DYNAMIC_TYPE.clone()]),
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        impls: vec![],
      }),
      values: Arc::new(vec![
        Calcit::List(Arc::new(CalcitList::Vector(vec![Calcit::tag("string")]))),
        Calcit::List(Arc::new(CalcitList::Vector(vec![]))),
      ]),
    };
    let enum_def = Arc::new(CalcitEnumDef::from_struct(prototype).expect("enum prototype"));
    assert!(matches!(enum_def.variants(), [EnumVariant { .. }, EnumVariant { .. }]));
    let shape =
      DataShapeGraph::build(&CalcitTypeAnnotation::Enum(enum_def.clone(), Arc::new(vec![])), "tests.edn").expect("derive enum shape");
    let input = cirru_edn::parse("%:: :ResultText :err |oops").expect("parse edn");
    let decoded = decode(&shape, &input).expect("decode enum");
    let Calcit::Enum(enum_value) = decoded else {
      panic!("expected enum");
    };
    assert!(enum_value.sum_type.as_ref().is_some_and(|actual| Arc::ptr_eq(actual, &enum_def)));

    let invalid = cirru_edn::parse("%:: :ResultText :ok 1").expect("parse invalid enum");
    let error = decode(&shape, &invalid).expect_err("arity must fail");
    assert!(error.message.contains("expects 0 payload(s), got 1"));
  }
}
