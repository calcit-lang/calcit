//! `encode-map-as`: the inverse of `decode-map-as` (#2562).
//!
//! A typed value is checked against the open data shape of its declared type
//! and turned into the plain shape `decode-map-as` reads back: Structs become
//! tag-keyed maps, `Option :some x` struct fields store `x` and `Option :none`
//! fields are left out. Enums keep their nominal value, as the decoder expects
//! them. Every result satisfies `decode-map-as (encode-map-as x T) T == x`.

use std::sync::Arc;

use crate::builtins::quick_build_atom;
use crate::calcit::data_shape::{DataShapeGraph, DataShapeNode, DataShapeValueError};
use crate::calcit::{Calcit, CalcitEnumValue, CalcitList};

const MAX_ENCODE_DEPTH: usize = 1024;

pub(crate) fn encode_map(shape: &DataShapeGraph, input: &Calcit) -> Result<Calcit, DataShapeValueError> {
  shape.validate_value(input)?;
  let encoded = MapEncoder { shape }.encode_node(shape.root, input, "$", 0)?;
  // Union members are picked by runtime kind; two struct members can both read
  // the same map, so confirm the decoder recovers the original value.
  if shape.nodes.iter().any(|node| matches!(node, DataShapeNode::Union { .. })) {
    match crate::data::edn_decode::decode_map(shape, &encoded) {
      Ok(decoded) if decoded == *input => {}
      _ => {
        return Err(DataShapeValueError::at(
          "$",
          "the encoded value would not decode back to the same value; a union has members that share one map shape",
        ));
      }
    }
  }
  Ok(encoded)
}

struct MapEncoder<'a> {
  shape: &'a DataShapeGraph,
}

impl MapEncoder<'_> {
  fn encode_node(&self, node_id: usize, input: &Calcit, path: &str, depth: usize) -> Result<Calcit, DataShapeValueError> {
    if depth > MAX_ENCODE_DEPTH {
      return Err(DataShapeValueError::at(path, format!("encode nesting exceeds {MAX_ENCODE_DEPTH}")));
    }
    let Some(node) = self.shape.nodes.get(node_id) else {
      return Err(DataShapeValueError::at(path, format!("invalid data shape node #{node_id}")));
    };
    match (node, input) {
      (DataShapeNode::MapOption { inner, .. }, Calcit::Enum(option)) => match option.extra.as_slice() {
        [item] => self.encode_node(*inner, item, path, depth + 1),
        // Outside a struct field there is no slot to leave out; the decoder
        // accepts the `Option :none` value itself.
        _ => Ok(input.to_owned()),
      },
      (DataShapeNode::List(inner), Calcit::List(items)) => {
        let mut values = Vec::with_capacity(items.len());
        for (idx, item) in items.iter().enumerate() {
          values.push(self.encode_node(*inner, item, &format!("{path}[{idx}]"), depth + 1)?);
        }
        Ok(Calcit::List(Arc::new(CalcitList::Vector(values))))
      }
      (DataShapeNode::Tuple(slots), Calcit::List(items)) => {
        let mut values = Vec::with_capacity(items.len());
        for (idx, (slot, item)) in slots.iter().zip(items.iter()).enumerate() {
          values.push(self.encode_node(*slot, item, &format!("{path}[{idx}]"), depth + 1)?);
        }
        Ok(Calcit::List(Arc::new(CalcitList::Vector(values))))
      }
      (DataShapeNode::Set(inner), Calcit::Set(items)) => {
        let mut values = rpds::HashTrieSet::new_sync();
        for item in items {
          let encoded = self.encode_node(*inner, item, &format!("{path}.item"), depth + 1)?;
          if values.contains(&encoded) {
            return Err(DataShapeValueError::at(path, "two set items encode to the same value"));
          }
          values.insert_mut(encoded);
        }
        Ok(Calcit::Set(values))
      }
      (DataShapeNode::Map { key, value }, Calcit::Map(items)) => {
        let mut values = rpds::HashTrieMap::new_sync();
        for (raw_key, raw_value) in items {
          let encoded_key = self.encode_node(*key, raw_key, &format!("{path}.key"), depth + 1)?;
          if values.contains_key(&encoded_key) {
            return Err(DataShapeValueError::at(path, "two map keys encode to the same value"));
          }
          let encoded_value = self.encode_node(*value, raw_value, &format!("{path}.value"), depth + 1)?;
          values.insert_mut(encoded_key, encoded_value);
        }
        Ok(Calcit::Map(values))
      }
      (DataShapeNode::Struct { fields, .. }, Calcit::Struct(actual)) => {
        let mut values = rpds::HashTrieMap::new_sync();
        for ((field, child), item) in fields.iter().zip(actual.values.iter()) {
          if matches!(self.shape.nodes.get(*child), Some(DataShapeNode::MapOption { .. }))
            && matches!(item, Calcit::Enum(option) if option.extra.is_empty())
          {
            continue;
          }
          let encoded = self.encode_node(*child, item, &format!("{path}.{}", field.ref_str()), depth + 1)?;
          values.insert_mut(Calcit::Tag(field.to_owned()), encoded);
        }
        Ok(Calcit::Map(values))
      }
      (DataShapeNode::Enum { variants, .. }, Calcit::Enum(enum_value)) => {
        let Calcit::Tag(tag) = enum_value.tag.as_ref() else {
          return Err(DataShapeValueError::at(path, "enum variant is not a tag"));
        };
        let payload_nodes = variants
          .iter()
          .find(|(candidate, _)| candidate == tag)
          .map(|(_, nodes)| nodes)
          .ok_or_else(|| DataShapeValueError::at(path, format!("unknown enum variant :{tag}")))?;
        let mut values = Vec::with_capacity(payload_nodes.len());
        for (idx, (payload, item)) in payload_nodes.iter().zip(enum_value.extra.iter()).enumerate() {
          values.push(self.encode_node(*payload, item, &format!("{path}.payload[{idx}]"), depth + 1)?);
        }
        if values.as_slice() == enum_value.extra.as_slice() {
          return Ok(input.to_owned());
        }
        Ok(Calcit::Enum(CalcitEnumValue {
          tag: enum_value.tag.clone(),
          extra: values.into(),
          sum_type: enum_value.sum_type.clone(),
        }))
      }
      (DataShapeNode::Union { members, .. }, _) => {
        let member = self
          .shape
          .union_candidates(members, input, false)
          .into_iter()
          .find(|member| self.shape.validate_node_value(*member, input, path, depth + 1).is_ok())
          .ok_or_else(|| DataShapeValueError::at(path, "value matches no union member"))?;
        self.encode_node(member, input, path, depth + 1)
      }
      (DataShapeNode::Ref(inner), Calcit::Ref(_, value_and_listeners)) => {
        let inner_value = value_and_listeners
          .lock()
          .map_err(|_| DataShapeValueError::at(path, "cannot read poisoned ref"))?
          .0
          .clone();
        Ok(quick_build_atom(self.encode_node(
          *inner,
          &inner_value,
          &format!("{path}.value"),
          depth + 1,
        )?))
      }
      // Scalars, Dynamic leaves and shapes already checked by `validate_value`.
      _ => Ok(input.to_owned()),
    }
  }
}
