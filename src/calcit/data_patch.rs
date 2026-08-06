use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use cirru_edn::EdnTag;

use super::data_shape::{DataShapeGraph, DataShapeNode};
use super::{Calcit, CalcitEnumValue, CalcitStructValue};

const MAX_PATCH_DEPTH: usize = 1024;

/// Compiler-owned patch for one closed `DataShapeGraph`.
///
/// The first slice deliberately covers only replacement and nominal aggregate
/// updates. Collection algorithms stay in Recollect until their strategies can
/// be bound to shape nodes without introducing Dynamic payloads here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DataPatch {
  version: u16,
  fingerprint: Arc<str>,
  root: DataPatchNode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DataPatchNode {
  Keep {
    node: usize,
  },
  Replace {
    node: usize,
    value: Calcit,
  },
  StructFields {
    node: usize,
    fields: Vec<(usize, EdnTag, DataPatchNode)>,
  },
  EnumPayload {
    node: usize,
    variant: usize,
    payloads: Vec<(usize, DataPatchNode)>,
  },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DataPatchError {
  pub(crate) path: String,
  pub(crate) message: String,
}

impl DataPatchError {
  fn at(path: impl Into<String>, message: impl Into<String>) -> Self {
    Self {
      path: path.into(),
      message: message.into(),
    }
  }
}

impl fmt::Display for DataPatchError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "data patch failed at {}: {}", self.path, self.message)
  }
}

impl DataPatch {
  pub(crate) fn new(shape: &DataShapeGraph, root: DataPatchNode) -> Result<Self, DataPatchError> {
    if root.node() != shape.root {
      return Err(DataPatchError::at(
        "$",
        format!("patch root node #{} does not match shape root #{}", root.node(), shape.root),
      ));
    }
    validate_patch_node(shape, &root, "$", 0)?;
    Ok(Self {
      version: shape.abi_version(),
      fingerprint: Arc::from(shape.fingerprint()),
      root,
    })
  }

  pub(crate) fn apply(&self, shape: &DataShapeGraph, base: &Calcit) -> Result<Calcit, DataPatchError> {
    if self.version != shape.abi_version() {
      return Err(DataPatchError::at(
        "$",
        format!(
          "patch data shape ABI version {} does not match {}",
          self.version,
          shape.abi_version()
        ),
      ));
    }
    if self.fingerprint.as_ref() != shape.fingerprint() {
      return Err(DataPatchError::at(
        "$",
        format!(
          "patch data shape fingerprint {} does not match {}",
          self.fingerprint,
          shape.fingerprint()
        ),
      ));
    }
    if self.root.node() != shape.root {
      return Err(DataPatchError::at(
        "$",
        format!("patch root node #{} does not match shape root #{}", self.root.node(), shape.root),
      ));
    }

    validate_patch_node(shape, &self.root, "$", 0)?;
    shape
      .validate_value(base)
      .map_err(|error| DataPatchError::at(error.path, format!("base value: {}", error.message)))?;
    let value = apply_patch_node(shape, &self.root, base, "$", 0)?;
    shape
      .validate_value(&value)
      .map_err(|error| DataPatchError::at(error.path, format!("patched value: {}", error.message)))?;
    Ok(value)
  }
}

impl DataPatchNode {
  fn node(&self) -> usize {
    match self {
      Self::Keep { node } | Self::Replace { node, .. } | Self::StructFields { node, .. } | Self::EnumPayload { node, .. } => *node,
    }
  }
}

fn validate_patch_node(shape: &DataShapeGraph, patch: &DataPatchNode, path: &str, depth: usize) -> Result<(), DataPatchError> {
  if depth > MAX_PATCH_DEPTH {
    return Err(DataPatchError::at(path, format!("patch nesting exceeds {MAX_PATCH_DEPTH}")));
  }
  let node_id = patch.node();
  let shape_node = shape
    .nodes
    .get(node_id)
    .ok_or_else(|| DataPatchError::at(path, format!("patch references missing shape node #{node_id}")))?;

  match patch {
    DataPatchNode::Keep { .. } => Ok(()),
    DataPatchNode::Replace { value, .. } => shape
      .validate_node_value(node_id, value, path, depth + 1)
      .map_err(|error| DataPatchError::at(error.path, format!("replacement value: {}", error.message))),
    DataPatchNode::StructFields { fields: updates, .. } => {
      let DataShapeNode::Struct { fields, .. } = shape_node else {
        return Err(DataPatchError::at(
          path,
          format!("StructFields requires a struct shape node, got #{node_id}"),
        ));
      };
      let mut seen = HashSet::new();
      for (field_index, field_tag, child_patch) in updates {
        if !seen.insert(*field_index) {
          return Err(DataPatchError::at(path, format!("duplicate struct field index {field_index}")));
        }
        let Some((expected_tag, child_node)) = fields.get(*field_index) else {
          return Err(DataPatchError::at(
            path,
            format!("struct field index {field_index} is out of bounds"),
          ));
        };
        if field_tag != expected_tag {
          return Err(DataPatchError::at(
            path,
            format!("struct field index {field_index} expects :{expected_tag}, got :{field_tag}"),
          ));
        }
        if child_patch.node() != *child_node {
          return Err(DataPatchError::at(
            format!("{path}.{}", field_tag.ref_str()),
            format!("field patch node #{} does not match child node #{child_node}", child_patch.node()),
          ));
        }
        validate_patch_node(shape, child_patch, &format!("{path}.{}", field_tag.ref_str()), depth + 1)?;
      }
      Ok(())
    }
    DataPatchNode::EnumPayload {
      variant,
      payloads: updates,
      ..
    } => {
      let DataShapeNode::Enum { variants, .. } = shape_node else {
        return Err(DataPatchError::at(
          path,
          format!("EnumPayload requires an enum shape node, got #{node_id}"),
        ));
      };
      let Some((variant_tag, payload_nodes)) = variants.get(*variant) else {
        return Err(DataPatchError::at(path, format!("enum variant index {variant} is out of bounds")));
      };
      let mut seen = HashSet::new();
      for (payload_index, child_patch) in updates {
        if !seen.insert(*payload_index) {
          return Err(DataPatchError::at(path, format!("duplicate enum payload index {payload_index}")));
        }
        let Some(child_node) = payload_nodes.get(*payload_index) else {
          return Err(DataPatchError::at(
            path,
            format!("enum :{variant_tag} payload index {payload_index} is out of bounds"),
          ));
        };
        if child_patch.node() != *child_node {
          return Err(DataPatchError::at(
            format!("{path}.payload[{payload_index}]"),
            format!("payload patch node #{} does not match child node #{child_node}", child_patch.node()),
          ));
        }
        validate_patch_node(shape, child_patch, &format!("{path}.payload[{payload_index}]"), depth + 1)?;
      }
      Ok(())
    }
  }
}

fn apply_patch_node(
  shape: &DataShapeGraph,
  patch: &DataPatchNode,
  base: &Calcit,
  path: &str,
  depth: usize,
) -> Result<Calcit, DataPatchError> {
  if depth > MAX_PATCH_DEPTH {
    return Err(DataPatchError::at(path, format!("patch nesting exceeds {MAX_PATCH_DEPTH}")));
  }
  match patch {
    DataPatchNode::Keep { .. } => Ok(base.clone()),
    DataPatchNode::Replace { value, .. } => Ok(value.clone()),
    DataPatchNode::StructFields { node, fields: updates } => {
      let DataShapeNode::Struct { fields, .. } = &shape.nodes[*node] else {
        return Err(DataPatchError::at(path, "StructFields reached a non-struct shape node"));
      };
      let Calcit::Struct(struct_value) = base else {
        return Err(DataPatchError::at(path, "StructFields base is not a struct"));
      };
      let mut values = struct_value.values.as_ref().clone();
      for (field_index, field_tag, child_patch) in updates {
        let (_, child_node) = fields
          .get(*field_index)
          .ok_or_else(|| DataPatchError::at(path, format!("struct field index {field_index} is out of bounds")))?;
        let field_value = values
          .get(*field_index)
          .ok_or_else(|| DataPatchError::at(path, format!("struct is missing field index {field_index}")))?
          .clone();
        let next = apply_patch_node(
          shape,
          child_patch,
          &field_value,
          &format!("{path}.{}", field_tag.ref_str()),
          depth + 1,
        )?;
        shape
          .validate_node_value(*child_node, &next, &format!("{path}.{}", field_tag.ref_str()), depth + 1)
          .map_err(|error| DataPatchError::at(error.path, error.message))?;
        values[*field_index] = next;
      }
      Ok(Calcit::Struct(CalcitStructValue {
        struct_ref: struct_value.struct_ref.clone(),
        values: Arc::new(values),
      }))
    }
    DataPatchNode::EnumPayload {
      node,
      variant,
      payloads: updates,
    } => {
      let DataShapeNode::Enum { variants, .. } = &shape.nodes[*node] else {
        return Err(DataPatchError::at(path, "EnumPayload reached a non-enum shape node"));
      };
      let Calcit::Enum(enum_value) = base else {
        return Err(DataPatchError::at(path, "EnumPayload base is not an enum"));
      };
      let (expected_tag, payload_nodes) = variants
        .get(*variant)
        .ok_or_else(|| DataPatchError::at(path, format!("enum variant index {variant} is out of bounds")))?;
      if !matches!(enum_value.tag.as_ref(), Calcit::Tag(actual) if actual == expected_tag) {
        return Err(DataPatchError::at(
          path,
          format!("base enum variant does not match :{expected_tag}; use Replace for variant changes"),
        ));
      }
      let mut payloads = enum_value.extra.clone();
      for (payload_index, child_patch) in updates {
        let child_node = payload_nodes
          .get(*payload_index)
          .ok_or_else(|| DataPatchError::at(path, format!("enum payload index {payload_index} is out of bounds")))?;
        let payload = payloads
          .get(*payload_index)
          .ok_or_else(|| DataPatchError::at(path, format!("enum value is missing payload index {payload_index}")))?
          .clone();
        let next = apply_patch_node(shape, child_patch, &payload, &format!("{path}.payload[{payload_index}]"), depth + 1)?;
        shape
          .validate_node_value(*child_node, &next, &format!("{path}.payload[{payload_index}]"), depth + 1)
          .map_err(|error| DataPatchError::at(error.path, error.message))?;
        payloads[*payload_index] = next;
      }
      Ok(Calcit::Enum(CalcitEnumValue {
        tag: enum_value.tag.clone(),
        extra: payloads,
        sum_type: enum_value.sum_type.clone(),
      }))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitEnumDef, CalcitList, CalcitStructDef, CalcitTypeAnnotation};

  fn point_fixture() -> (DataShapeGraph, Arc<CalcitStructDef>, Calcit) {
    let mut nominal = CalcitStructDef::from_fields(EdnTag::new("Point"), vec![EdnTag::new("x"), EdnTag::new("label")]);
    nominal.field_types = Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)]);
    let nominal = Arc::new(nominal);
    let shape =
      DataShapeGraph::build(&CalcitTypeAnnotation::Struct(nominal.clone(), Arc::new(vec![])), "tests.patch").expect("point data shape");
    let value = Calcit::Struct(CalcitStructValue {
      struct_ref: nominal.clone(),
      values: Arc::new(vec![Calcit::Number(1.0), Calcit::Str(Arc::from("old"))]),
    });
    (shape, nominal, value)
  }

  #[test]
  fn applies_struct_field_patch_and_preserves_nominal_identity() {
    let (shape, _, base) = point_fixture();
    let DataShapeNode::Struct { fields, .. } = &shape.nodes[shape.root] else {
      panic!("point root must be a struct");
    };
    let x_node = fields[0].1;
    let patch = DataPatch::new(
      &shape,
      DataPatchNode::StructFields {
        node: shape.root,
        fields: vec![(
          0,
          EdnTag::new("x"),
          DataPatchNode::Replace {
            node: x_node,
            value: Calcit::Number(2.0),
          },
        )],
      },
    )
    .expect("valid point patch");

    let value = patch.apply(&shape, &base).expect("apply point patch");
    let (Calcit::Struct(before), Calcit::Struct(after)) = (&base, &value) else {
      panic!("point values must be structs");
    };
    assert!(Arc::ptr_eq(&before.struct_ref, &after.struct_ref));
    assert_eq!(after.values.as_ref(), &[Calcit::Number(2.0), Calcit::Str(Arc::from("old"))]);
  }

  #[test]
  fn rejects_wrong_struct_field_tag_and_replacement_type() {
    let (shape, _, _) = point_fixture();
    let DataShapeNode::Struct { fields, .. } = &shape.nodes[shape.root] else {
      panic!("point root must be a struct");
    };
    let x_node = fields[0].1;
    let wrong_tag = DataPatch::new(
      &shape,
      DataPatchNode::StructFields {
        node: shape.root,
        fields: vec![(0, EdnTag::new("label"), DataPatchNode::Keep { node: x_node })],
      },
    )
    .expect_err("field tag must agree with its index");
    assert!(wrong_tag.to_string().contains("expects :x"));

    let wrong_value = DataPatch::new(
      &shape,
      DataPatchNode::StructFields {
        node: shape.root,
        fields: vec![(
          0,
          EdnTag::new("x"),
          DataPatchNode::Replace {
            node: x_node,
            value: Calcit::Str(Arc::from("wrong")),
          },
        )],
      },
    )
    .expect_err("replacement must match its child shape");
    assert!(wrong_value.to_string().contains("expected number"));
  }

  #[test]
  fn rejects_patch_from_another_shape_fingerprint() {
    let (shape, _, base) = point_fixture();
    let patch = DataPatch {
      version: shape.abi_version(),
      fingerprint: Arc::from("another-shape"),
      root: DataPatchNode::Keep { node: shape.root },
    };
    let error = patch.apply(&shape, &base).expect_err("fingerprint mismatch must fail");
    assert!(error.to_string().contains("fingerprint"));
  }

  #[test]
  fn applies_payload_patch_only_to_the_declared_enum_variant() {
    let enum_record = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(
        EdnTag::new("Outcome"),
        vec![EdnTag::new("none"), EdnTag::new("score")],
      )),
      values: Arc::new(vec![
        Calcit::List(Arc::new(CalcitList::default())),
        Calcit::List(Arc::new(CalcitList::from([CalcitTypeAnnotation::Number.to_calcit()].as_slice()))),
      ]),
    };
    let nominal = Arc::new(CalcitEnumDef::from_record(enum_record).expect("outcome enum"));
    let shape =
      DataShapeGraph::build(&CalcitTypeAnnotation::Enum(nominal.clone(), Arc::new(vec![])), "tests.patch").expect("outcome shape");
    let DataShapeNode::Enum { variants, .. } = &shape.nodes[shape.root] else {
      panic!("outcome root must be an enum");
    };
    let variant = variants.iter().position(|(tag, _)| tag.ref_str() == "score").unwrap();
    let score_node = variants[variant].1[0];
    let base = Calcit::Enum(CalcitEnumValue {
      tag: Arc::new(Calcit::Tag(EdnTag::new("score"))),
      extra: vec![Calcit::Number(1.0)],
      sum_type: Some(nominal),
    });
    let patch = DataPatch::new(
      &shape,
      DataPatchNode::EnumPayload {
        node: shape.root,
        variant,
        payloads: vec![(
          0,
          DataPatchNode::Replace {
            node: score_node,
            value: Calcit::Number(2.0),
          },
        )],
      },
    )
    .expect("valid enum payload patch");

    let value = patch.apply(&shape, &base).expect("apply enum patch");
    let Calcit::Enum(enum_value) = value else {
      panic!("patched outcome must remain an enum");
    };
    assert_eq!(enum_value.extra, vec![Calcit::Number(2.0)]);
    assert!(enum_value.sum_type.is_some());
  }
}
