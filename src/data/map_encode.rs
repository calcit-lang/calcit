//! `encode-map-as`: the inverse of `decode-map-as` (#2562).
//!
//! A typed value is checked against the open data shape of its declared type
//! and turned into the plain shape `decode-map-as` reads back: Structs become
//! tag-keyed maps, `Option :some x` struct fields store `x` and `Option :none`
//! fields are left out. Enums keep their nominal value, as the decoder expects
//! them. Decoding the result gives back an equal value; a typed `Ref` is
//! rebuilt around its encoded content, so only its identity differs.

use std::sync::Arc;

use crate::builtins::quick_build_atom;
use crate::calcit::data_shape::{DataShapeGraph, DataShapeNode, DataShapeValueError};
use crate::calcit::{Calcit, CalcitEnumValue, CalcitList};

const MAX_ENCODE_DEPTH: usize = 1024;

pub(crate) fn encode_map(shape: &DataShapeGraph, input: &Calcit) -> Result<Calcit, DataShapeValueError> {
  shape.validate_value(input)?;
  MapEncoder { shape }.encode_node(shape.root, input, "$", 0)
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
      (DataShapeNode::MapOption { .. }, Calcit::Enum(option)) => self.encode_option(node_id, option, false, path, depth),
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
        let encoded = match (self.shape.nodes.get(member), input) {
          // A bare payload would be read as another member; keep the Option value.
          (Some(DataShapeNode::MapOption { .. }), Calcit::Enum(option)) => self.encode_option(member, option, true, path, depth)?,
          _ => self.encode_node(member, input, path, depth + 1)?,
        };
        // The decoder tries members in this order and keeps the first that
        // fits; an earlier fit would turn the value into another member.
        for candidate in self.shape.union_candidates(members, &encoded, true) {
          if candidate == member {
            return Ok(encoded);
          }
          if crate::data::edn_decode::decode_map_node(self.shape, candidate, &encoded).is_ok() {
            break;
          }
        }
        Err(DataShapeValueError::at(
          path,
          "the encoded value would decode as another union member; the union has members that share one shape",
        ))
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

  /// `Option :some x` encodes as `x`, unless `keep` is set or `x` encodes to an
  /// Option value itself, which the decoder would read as this Option.
  /// Outside a struct field there is no slot to leave out, so `Option :none`
  /// stays as the value the decoder accepts.
  fn encode_option(
    &self,
    node_id: usize,
    option: &CalcitEnumValue,
    keep: bool,
    path: &str,
    depth: usize,
  ) -> Result<Calcit, DataShapeValueError> {
    let Some(DataShapeNode::MapOption { nominal, inner, .. }) = self.shape.nodes.get(node_id) else {
      return Err(DataShapeValueError::at(path, format!("invalid Option shape node #{node_id}")));
    };
    let [item] = option.extra.as_slice() else {
      return Ok(Calcit::Enum(option.to_owned()));
    };
    let encoded = self.encode_node(*inner, item, path, depth + 1)?;
    let reads_as_option = matches!(&encoded, Calcit::Enum(value)
      if value.sum_type.as_ref().is_some_and(|actual| Arc::ptr_eq(actual, nominal) || actual.same_nominal_definition(nominal)));
    if keep || reads_as_option {
      return Ok(Calcit::Enum(CalcitEnumValue {
        tag: option.tag.clone(),
        extra: vec![encoded].into(),
        sum_type: option.sum_type.clone(),
      }));
    }
    Ok(encoded)
  }
}
