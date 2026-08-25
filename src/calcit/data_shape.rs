use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use cirru_edn::EdnTag;
use md5::{Digest, Md5};

use super::type_annotation::{TypeBindings, validate_runtime_generic_where_bounds};
use super::{Calcit, CalcitEnumDef, CalcitStructDef, CalcitTypeAnnotation};
use crate::program;

const DATA_SHAPE_ABI_VERSION: u16 = 2;
const MAX_SHAPE_VALUE_DEPTH: usize = 1024;

/// A closed, backend-neutral description of statically typed Calcit data.
///
/// The graph is compiler-owned and deliberately excludes Dynamic and other
/// open host values. Format-specific decoders and future typed diff/patch
/// executors consume this graph rather than resolving type expressions again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DataShapeGraph {
  pub(crate) root: usize,
  pub(crate) nodes: Vec<DataShapeNode>,
  fingerprint: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DataShapeNode {
  /// An intentionally open value at a runtime data boundary. Closed EDN
  /// shapes never contain this node.
  Dynamic,
  Nil,
  Unit,
  Bool,
  Number,
  String,
  Symbol,
  Tag,
  Buffer,
  CirruQuote,
  Optional(usize),
  /// A nominal `Option<T>` accepted from an ordinary runtime value. Missing
  /// struct fields become `%none`; present raw values become `%some value`.
  MapOption {
    nominal: Arc<CalcitEnumDef>,
    nominal_path: Option<(Arc<str>, Arc<str>)>,
    inner: usize,
  },
  List(usize),
  Set(usize),
  Map {
    key: usize,
    value: usize,
  },
  Ref(usize),
  Struct {
    nominal: Arc<CalcitStructDef>,
    nominal_path: Option<(Arc<str>, Arc<str>)>,
    type_args: Arc<Vec<Arc<CalcitTypeAnnotation>>>,
    fields: Vec<(EdnTag, usize)>,
  },
  Enum {
    nominal: Arc<CalcitEnumDef>,
    nominal_path: Option<(Arc<str>, Arc<str>)>,
    type_args: Arc<Vec<Arc<CalcitTypeAnnotation>>>,
    variants: Vec<(EdnTag, Vec<usize>)>,
  },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DataShapeError {
  message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DataShapeValueError {
  pub(crate) path: String,
  pub(crate) message: String,
}

impl DataShapeError {
  fn new(message: impl Into<String>) -> Self {
    Self { message: message.into() }
  }
}

impl fmt::Display for DataShapeError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.message)
  }
}

impl DataShapeValueError {
  fn at(path: impl Into<String>, message: impl Into<String>) -> Self {
    Self {
      path: path.into(),
      message: message.into(),
    }
  }
}

impl fmt::Display for DataShapeValueError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "data shape validation failed at {}: {}", self.path, self.message)
  }
}

struct GraphBuilder {
  nodes: Vec<Option<DataShapeNode>>,
  nominal_nodes: HashMap<String, usize>,
  resolving_aliases: HashSet<String>,
  resolving_slots: HashSet<String>,
  allow_dynamic: bool,
}

impl DataShapeGraph {
  pub(crate) fn build(target: &CalcitTypeAnnotation, default_ns: &str) -> Result<Self, DataShapeError> {
    Self::build_with_options(target, default_ns, false)
  }

  /// Builds a decoder shape for a runtime Calcit map. This intentionally
  /// permits explicit Dynamic leaves and lifts raw option fields.
  pub(crate) fn build_open(target: &CalcitTypeAnnotation, default_ns: &str) -> Result<Self, DataShapeError> {
    Self::build_with_options(target, default_ns, true)
  }

  fn build_with_options(target: &CalcitTypeAnnotation, default_ns: &str, allow_dynamic: bool) -> Result<Self, DataShapeError> {
    let mut builder = GraphBuilder {
      nodes: vec![],
      nominal_nodes: HashMap::new(),
      resolving_aliases: HashSet::new(),
      resolving_slots: HashSet::new(),
      allow_dynamic,
    };
    let root = builder.build_type(target, default_ns)?;
    let nodes = builder
      .nodes
      .into_iter()
      .enumerate()
      .map(|(idx, node)| node.ok_or_else(|| DataShapeError::new(format!("data shape node #{idx} was not completed"))))
      .collect::<Result<Vec<_>, _>>()?;
    Self::from_nodes(root, nodes)
  }

  pub(crate) fn from_nodes(root: usize, nodes: Vec<DataShapeNode>) -> Result<Self, DataShapeError> {
    if nodes.get(root).is_none() {
      return Err(DataShapeError::new(format!("data shape root #{root} is out of bounds")));
    }
    for (node_id, node) in nodes.iter().enumerate() {
      for child in node.child_nodes() {
        if nodes.get(child).is_none() {
          return Err(DataShapeError::new(format!(
            "data shape node #{node_id} references missing child #{child}"
          )));
        }
      }
    }
    let fingerprint = Arc::from(shape_fingerprint(root, &nodes));
    Ok(Self { root, nodes, fingerprint })
  }

  pub(crate) fn fingerprint(&self) -> &str {
    &self.fingerprint
  }

  pub(crate) fn abi_version(&self) -> u16 {
    DATA_SHAPE_ABI_VERSION
  }

  pub(crate) fn into_calcit_handle(self) -> Calcit {
    Calcit::AnyRef(cirru_edn::EdnAnyRef::new(Arc::new(self)))
  }

  pub(crate) fn from_calcit_handle(value: &Calcit) -> Option<Arc<Self>> {
    let Calcit::AnyRef(reference) = value else {
      return None;
    };
    let guard = reference.0.read().ok()?;
    guard.as_any().downcast_ref::<Arc<Self>>().cloned()
  }

  pub(crate) fn nominal_paths(&self) -> Vec<(Arc<str>, Arc<str>)> {
    let mut paths = Vec::new();
    for node in &self.nodes {
      let path = match node {
        DataShapeNode::Struct { nominal_path, .. }
        | DataShapeNode::Enum { nominal_path, .. }
        | DataShapeNode::MapOption { nominal_path, .. } => nominal_path,
        _ => continue,
      };
      if let Some(path) = path
        && !paths.contains(path)
      {
        paths.push(path.clone());
      }
    }
    paths
  }

  pub(crate) fn validate_value(&self, value: &Calcit) -> Result<(), DataShapeValueError> {
    self.validate_node_value(self.root, value, "$", 0)
  }

  pub(crate) fn validate_node_value(
    &self,
    node_id: usize,
    value: &Calcit,
    path: &str,
    depth: usize,
  ) -> Result<(), DataShapeValueError> {
    if depth > MAX_SHAPE_VALUE_DEPTH {
      return Err(DataShapeValueError::at(
        path,
        format!("value nesting exceeds {MAX_SHAPE_VALUE_DEPTH}"),
      ));
    }
    let node = self
      .nodes
      .get(node_id)
      .ok_or_else(|| DataShapeValueError::at(path, format!("missing shape node #{node_id}")))?;

    match node {
      DataShapeNode::Nil if matches!(value, Calcit::Nil) => Ok(()),
      DataShapeNode::Unit if matches!(value, Calcit::Unit) => Ok(()),
      DataShapeNode::Bool if matches!(value, Calcit::Bool(_)) => Ok(()),
      DataShapeNode::Number if matches!(value, Calcit::Number(_)) => Ok(()),
      DataShapeNode::String if matches!(value, Calcit::Str(_)) => Ok(()),
      DataShapeNode::Symbol if matches!(value, Calcit::Symbol { .. }) => Ok(()),
      DataShapeNode::Tag if matches!(value, Calcit::Tag(_)) => Ok(()),
      DataShapeNode::Buffer if matches!(value, Calcit::Buffer(_)) => Ok(()),
      DataShapeNode::CirruQuote if matches!(value, Calcit::CirruQuote(_)) => Ok(()),
      DataShapeNode::Optional(inner) => {
        if matches!(value, Calcit::Nil) {
          Ok(())
        } else {
          self.validate_node_value(*inner, value, path, depth + 1)
        }
      }
      DataShapeNode::Dynamic => Ok(()),
      DataShapeNode::MapOption { nominal, inner, .. } => match value {
        Calcit::Enum(enum_value)
          if enum_value
            .sum_type
            .as_ref()
            .is_some_and(|actual| Arc::ptr_eq(actual, nominal) || actual.name() == nominal.name()) =>
        {
          match (enum_value.tag.as_ref(), enum_value.extra.as_slice()) {
            (Calcit::Tag(tag), []) if tag.ref_str() == "none" => Ok(()),
            (Calcit::Tag(tag), [item]) if tag.ref_str() == "some" => self.validate_node_value(*inner, item, path, depth + 1),
            _ => Err(DataShapeValueError::at(path, "invalid Option value")),
          }
        }
        _ => Err(shape_kind_mismatch(path, "Option", value)),
      },
      DataShapeNode::List(inner) => {
        let Calcit::List(values) = value else {
          return Err(shape_kind_mismatch(path, "list", value));
        };
        for (idx, item) in values.iter().enumerate() {
          self.validate_node_value(*inner, item, &format!("{path}[{idx}]"), depth + 1)?;
        }
        Ok(())
      }
      DataShapeNode::Set(inner) => {
        let Calcit::Set(values) = value else {
          return Err(shape_kind_mismatch(path, "set", value));
        };
        for item in values.iter() {
          self.validate_node_value(*inner, item, &format!("{path}.item"), depth + 1)?;
        }
        Ok(())
      }
      DataShapeNode::Map { key, value: value_node } => {
        let Calcit::Map(values) = value else {
          return Err(shape_kind_mismatch(path, "map", value));
        };
        for (item_key, item_value) in values.iter() {
          self.validate_node_value(*key, item_key, &format!("{path}.key"), depth + 1)?;
          self.validate_node_value(*value_node, item_value, &format!("{path}.value"), depth + 1)?;
        }
        Ok(())
      }
      DataShapeNode::Ref(inner) => {
        let Calcit::Ref(_, value_and_listeners) = value else {
          return Err(shape_kind_mismatch(path, "ref", value));
        };
        let inner_value = value_and_listeners
          .lock()
          .map_err(|_| DataShapeValueError::at(path, "cannot read poisoned ref"))?
          .0
          .clone();
        self.validate_node_value(*inner, &inner_value, &format!("{path}.value"), depth + 1)
      }
      DataShapeNode::Struct { nominal, fields, .. } => {
        let Calcit::Struct(struct_value) = value else {
          return Err(shape_kind_mismatch(path, &format!("struct :{}", nominal.name), value));
        };
        if !Arc::ptr_eq(&struct_value.struct_ref, nominal) {
          return Err(DataShapeValueError::at(
            path,
            format!("expected nominal struct :{}, got :{}", nominal.name, struct_value.struct_ref.name),
          ));
        }
        if struct_value.values.len() != fields.len() {
          return Err(DataShapeValueError::at(
            path,
            format!(
              "struct :{} expects {} value(s), got {}",
              nominal.name,
              fields.len(),
              struct_value.values.len()
            ),
          ));
        }
        for (idx, ((field, child), item)) in fields.iter().zip(struct_value.values.iter()).enumerate() {
          if struct_value.struct_ref.fields.get(idx) != Some(field) {
            return Err(DataShapeValueError::at(
              path,
              format!("struct :{} field #{idx} does not match :{field}", nominal.name),
            ));
          }
          self.validate_node_value(*child, item, &format!("{path}.{}", field.ref_str()), depth + 1)?;
        }
        Ok(())
      }
      DataShapeNode::Enum { nominal, variants, .. } => {
        let Calcit::Enum(enum_value) = value else {
          return Err(shape_kind_mismatch(path, &format!("enum :{}", nominal.name()), value));
        };
        let Some(actual_enum) = enum_value.sum_type.as_ref() else {
          return Err(DataShapeValueError::at(
            path,
            format!("expected nominal enum :{}, got anonymous enum", nominal.name()),
          ));
        };
        if !Arc::ptr_eq(actual_enum, nominal) {
          return Err(DataShapeValueError::at(
            path,
            format!("expected nominal enum :{}, got :{}", nominal.name(), actual_enum.name()),
          ));
        }
        let Calcit::Tag(tag) = enum_value.tag.as_ref() else {
          return Err(DataShapeValueError::at(path, "enum variant is not a tag"));
        };
        let Some((_, payload_nodes)) = variants.iter().find(|(candidate, _)| candidate == tag) else {
          return Err(DataShapeValueError::at(
            path,
            format!("enum :{} has no variant :{tag}", nominal.name()),
          ));
        };
        if enum_value.extra.len() != payload_nodes.len() {
          return Err(DataShapeValueError::at(
            path,
            format!(
              "enum :{} variant :{tag} expects {} payload(s), got {}",
              nominal.name(),
              payload_nodes.len(),
              enum_value.extra.len()
            ),
          ));
        }
        for (idx, (child, item)) in payload_nodes.iter().zip(enum_value.extra.iter()).enumerate() {
          self.validate_node_value(*child, item, &format!("{path}.payload[{idx}]"), depth + 1)?;
        }
        Ok(())
      }
      _ => Err(shape_kind_mismatch(path, node.expected_kind(), value)),
    }
  }
}

impl DataShapeNode {
  fn child_nodes(&self) -> Vec<usize> {
    match self {
      Self::Dynamic => vec![],
      Self::Optional(inner) | Self::List(inner) | Self::Set(inner) | Self::Ref(inner) => vec![*inner],
      Self::MapOption { inner, .. } => vec![*inner],
      Self::Map { key, value } => vec![*key, *value],
      Self::Struct { fields, .. } => fields.iter().map(|(_, node)| *node).collect(),
      Self::Enum { variants, .. } => variants.iter().flat_map(|(_, payloads)| payloads.iter().copied()).collect(),
      _ => vec![],
    }
  }

  pub(crate) fn expected_kind(&self) -> &str {
    match self {
      Self::Dynamic => "dynamic",
      Self::Nil => "nil",
      Self::Unit => "&unit",
      Self::Bool => "bool",
      Self::Number => "number",
      Self::String => "string",
      Self::Symbol => "symbol",
      Self::Tag => "tag",
      Self::Buffer => "buffer",
      Self::CirruQuote => "cirru-quote",
      Self::MapOption { .. } => "Option",
      Self::Optional(_) => "optional value",
      Self::List(_) => "list",
      Self::Set(_) => "set",
      Self::Map { .. } => "map",
      Self::Ref(_) => "ref",
      Self::Struct { .. } => "struct",
      Self::Enum { .. } => "enum",
    }
  }
}

fn shape_kind_mismatch(path: &str, expected: &str, actual: &Calcit) -> DataShapeValueError {
  DataShapeValueError::at(
    path,
    format!("expected {expected}, got {}", crate::calcit::brief_type_of_value(actual)),
  )
}

impl GraphBuilder {
  fn push(&mut self, node: DataShapeNode) -> usize {
    let id = self.nodes.len();
    self.nodes.push(Some(node));
    id
  }

  fn build_type(&mut self, target: &CalcitTypeAnnotation, default_ns: &str) -> Result<usize, DataShapeError> {
    match target {
      CalcitTypeAnnotation::Nil => Ok(self.push(DataShapeNode::Nil)),
      CalcitTypeAnnotation::Unit => Ok(self.push(DataShapeNode::Unit)),
      CalcitTypeAnnotation::Bool => Ok(self.push(DataShapeNode::Bool)),
      CalcitTypeAnnotation::Number => Ok(self.push(DataShapeNode::Number)),
      CalcitTypeAnnotation::String => Ok(self.push(DataShapeNode::String)),
      CalcitTypeAnnotation::Symbol => Ok(self.push(DataShapeNode::Symbol)),
      CalcitTypeAnnotation::Tag => Ok(self.push(DataShapeNode::Tag)),
      CalcitTypeAnnotation::Buffer => Ok(self.push(DataShapeNode::Buffer)),
      CalcitTypeAnnotation::CirruQuote => Ok(self.push(DataShapeNode::CirruQuote)),
      CalcitTypeAnnotation::Optional(inner) => {
        let inner = self.build_type(inner, default_ns)?;
        Ok(self.push(DataShapeNode::Optional(inner)))
      }
      CalcitTypeAnnotation::List(inner) => {
        let inner = self.build_type(inner, default_ns)?;
        Ok(self.push(DataShapeNode::List(inner)))
      }
      CalcitTypeAnnotation::Set(inner) => {
        let inner = self.build_type(inner, default_ns)?;
        Ok(self.push(DataShapeNode::Set(inner)))
      }
      CalcitTypeAnnotation::Map(key, value) => {
        let key = self.build_type(key, default_ns)?;
        let value = self.build_type(value, default_ns)?;
        Ok(self.push(DataShapeNode::Map { key, value }))
      }
      CalcitTypeAnnotation::Ref(inner) => {
        let inner = self.build_type(inner, default_ns)?;
        Ok(self.push(DataShapeNode::Ref(inner)))
      }
      CalcitTypeAnnotation::Struct(nominal, args) => {
        let path = infer_nominal_path(default_ns, nominal.name.ref_str());
        self.build_struct(nominal.clone(), args, path, default_ns)
      }
      CalcitTypeAnnotation::StructValue(nominal) => {
        let path = infer_nominal_path(default_ns, nominal.name.ref_str());
        self.build_struct(nominal.clone(), &Arc::new(vec![]), path, default_ns)
      }
      CalcitTypeAnnotation::Enum(nominal, args) => {
        let path = infer_nominal_path(default_ns, nominal.name().ref_str());
        self.build_enum(nominal.clone(), args, path, default_ns)
      }
      CalcitTypeAnnotation::EnumValue(nominal) => {
        let path = infer_nominal_path(default_ns, nominal.name().ref_str());
        self.build_enum(nominal.clone(), &Arc::new(vec![]), path, default_ns)
      }
      CalcitTypeAnnotation::StructDef(_) | CalcitTypeAnnotation::EnumDef(_) => Err(unsupported_type(
        "type-definition values are not closed application data; use the corresponding Struct or Enum instance type",
      )),
      CalcitTypeAnnotation::TypeRef(name, args) if self.allow_dynamic && target.is_option_type() => {
        let Some(inner) = args.first() else {
          return Err(DataShapeError::new("Option requires one type argument"));
        };
        let (ns, def) = qualify_type_ref(name, default_ns);
        if ns.as_ref() != super::CORE_NS || def.as_ref() != "Option" {
          return Err(DataShapeError::new("Option must resolve to calcit.core/Option"));
        }
        if inner.is_option_type() {
          return Err(DataShapeError::new("nested Option<Option<T>> is not supported by decode-map-as"));
        }
        let qualified = CalcitTypeAnnotation::TypeRef(Arc::from(format!("{ns}/{def}")), args.clone());
        let Some(nominal) = qualified.resolve_to_enum() else {
          return Err(DataShapeError::new("Option must resolve to calcit.core/Option"));
        };
        let inner = self.build_type(inner, default_ns)?;
        Ok(self.push(DataShapeNode::MapOption {
          nominal: Arc::new(nominal),
          nominal_path: Some((ns, def)),
          inner,
        }))
      }
      CalcitTypeAnnotation::TypeRef(name, args) => self.build_named(name, args, default_ns),
      CalcitTypeAnnotation::TypeSlot(name) => match super::resolve_type_slot(name) {
        Some(resolved) => {
          let slot_key = name.to_string();
          if !self.resolving_slots.insert(slot_key.clone()) {
            return Err(DataShapeError::new(format!("recursive type slot `*{name}`")));
          }
          let result = self.build_type(&resolved, default_ns);
          self.resolving_slots.remove(&slot_key);
          result
        }
        None => Err(DataShapeError::new(format!("unbound type slot `*{name}`"))),
      },
      CalcitTypeAnnotation::Dynamic if self.allow_dynamic => Ok(self.push(DataShapeNode::Dynamic)),
      CalcitTypeAnnotation::Dynamic => Err(unsupported_type("Dynamic is forbidden in a closed data shape")),
      CalcitTypeAnnotation::TypeVar(name) => Err(unsupported_type(&format!("generic variable '{name} is not bound"))),
      CalcitTypeAnnotation::AnonymousEnum => Err(unsupported_type("anonymous enum has no declared enum schema")),
      CalcitTypeAnnotation::DynFn | CalcitTypeAnnotation::Fn(_) => Err(unsupported_type("function values are not closed data")),
      CalcitTypeAnnotation::Macro(_) | CalcitTypeAnnotation::Syntax(_) => {
        Err(unsupported_type("compile-time macro syntax contracts are not application data"))
      }
      CalcitTypeAnnotation::Trait(_) | CalcitTypeAnnotation::TraitSet(_) => {
        Err(unsupported_type("trait constraints are not data shapes"))
      }
      CalcitTypeAnnotation::JsObject => Err(unsupported_type("JsObject is an opaque host value")),
      CalcitTypeAnnotation::JsNullish(_) => Err(unsupported_type("JsNullish is an opaque JavaScript boundary value")),
      CalcitTypeAnnotation::Custom(value) => Err(unsupported_type(&format!("custom type `{value}` has no data shape"))),
      CalcitTypeAnnotation::Variadic(_) => Err(unsupported_type("Variadic is a function parameter constraint")),
    }
  }

  fn build_named(&mut self, name: &str, args: &Arc<Vec<Arc<CalcitTypeAnnotation>>>, default_ns: &str) -> Result<usize, DataShapeError> {
    let (ns, def) = qualify_type_ref(name, default_ns);
    let qualified = CalcitTypeAnnotation::TypeRef(Arc::from(format!("{ns}/{def}")), args.clone());
    if let Some(struct_def) = qualified.resolve_to_struct() {
      return self.build_struct(Arc::new(struct_def), args, Some((ns, def)), default_ns);
    }
    if let Some(enum_def) = qualified.resolve_to_enum() {
      return self.build_enum(Arc::new(enum_def), args, Some((ns, def)), default_ns);
    }

    let schema = program::lookup_def_schema(&ns, &def);
    if !matches!(schema.as_ref(), CalcitTypeAnnotation::Dynamic)
      && !matches!(schema.as_ref(), CalcitTypeAnnotation::TypeRef(schema_name, _) if schema_name.as_ref() == name)
    {
      if !args.is_empty() {
        return Err(DataShapeError::new(format!(
          "cannot apply generic arguments to type alias `{ns}/{def}`"
        )));
      }
      let alias_key = format!("{ns}/{def}");
      if !self.resolving_aliases.insert(alias_key.clone()) {
        return Err(DataShapeError::new(format!(
          "recursive type alias `{alias_key}`; recursive data must use a nominal struct or enum"
        )));
      }
      let result = self.build_type(schema.as_ref(), &ns);
      self.resolving_aliases.remove(&alias_key);
      return result;
    }

    Err(DataShapeError::new(format!("cannot resolve named type `{ns}/{def}`")))
  }

  fn build_struct(
    &mut self,
    nominal: Arc<CalcitStructDef>,
    args: &Arc<Vec<Arc<CalcitTypeAnnotation>>>,
    nominal_path: Option<(Arc<str>, Arc<str>)>,
    default_ns: &str,
  ) -> Result<usize, DataShapeError> {
    validate_generic_application(&nominal.name.to_string(), &nominal.generics, args)?;
    if nominal.fields.len() != nominal.field_types.len() {
      return Err(DataShapeError::new(format!(
        "cannot derive `{}`: {} fields but {} field types",
        nominal.name,
        nominal.fields.len(),
        nominal.field_types.len()
      )));
    }
    let owner_ns = nominal_path.as_ref().map(|(ns, _)| ns.as_ref()).unwrap_or(default_ns);
    for arg in args.iter() {
      self.build_type(arg, owner_ns)?;
    }
    let bindings = generic_bindings(&nominal.generics, args);
    validate_runtime_generic_where_bounds(&bindings, nominal.where_bounds.as_ref())
      .map_err(|message| DataShapeError::new(format!("cannot derive `{}`: {message}", nominal.name)))?;
    let key = nominal_key("struct", &nominal.name, args, nominal_path.as_ref(), owner_ns);
    if let Some(existing) = self.nominal_nodes.get(&key) {
      return Ok(*existing);
    }
    let node_id = self.nodes.len();
    self.nodes.push(None);
    self.nominal_nodes.insert(key, node_id);

    let mut fields = Vec::with_capacity(nominal.fields.len());
    for (field, field_type) in nominal.fields.iter().zip(nominal.field_types.iter()) {
      let resolved = field_type.substitute_type_vars(&bindings);
      let field_node = self.build_type(resolved.as_ref(), owner_ns)?;
      fields.push((field.clone(), field_node));
    }
    self.nodes[node_id] = Some(DataShapeNode::Struct {
      nominal,
      nominal_path,
      type_args: args.clone(),
      fields,
    });
    Ok(node_id)
  }

  fn build_enum(
    &mut self,
    nominal: Arc<CalcitEnumDef>,
    args: &Arc<Vec<Arc<CalcitTypeAnnotation>>>,
    nominal_path: Option<(Arc<str>, Arc<str>)>,
    default_ns: &str,
  ) -> Result<usize, DataShapeError> {
    validate_generic_application(&nominal.name().to_string(), nominal.generics(), args)?;
    let owner_ns = nominal_path.as_ref().map(|(ns, _)| ns.as_ref()).unwrap_or(default_ns);
    for arg in args.iter() {
      self.build_type(arg, owner_ns)?;
    }
    let bindings = generic_bindings(nominal.generics(), args);
    validate_runtime_generic_where_bounds(&bindings, nominal.where_bounds())
      .map_err(|message| DataShapeError::new(format!("cannot derive `{}`: {message}", nominal.name())))?;
    let key = nominal_key("enum", nominal.name(), args, nominal_path.as_ref(), owner_ns);
    if let Some(existing) = self.nominal_nodes.get(&key) {
      return Ok(*existing);
    }
    let node_id = self.nodes.len();
    self.nodes.push(None);
    self.nominal_nodes.insert(key, node_id);

    let mut variants = Vec::with_capacity(nominal.variants().len());
    for variant in nominal.variants() {
      let mut payloads = Vec::with_capacity(variant.payload_types().len());
      for payload_type in variant.payload_types() {
        let resolved = payload_type.substitute_type_vars(&bindings);
        payloads.push(self.build_type(resolved.as_ref(), owner_ns)?);
      }
      variants.push((variant.tag.clone(), payloads));
    }
    self.nodes[node_id] = Some(DataShapeNode::Enum {
      nominal,
      nominal_path,
      type_args: args.clone(),
      variants,
    });
    Ok(node_id)
  }
}

fn validate_generic_application(name: &str, generics: &[Arc<str>], args: &[Arc<CalcitTypeAnnotation>]) -> Result<(), DataShapeError> {
  if generics.len() != args.len() {
    return Err(DataShapeError::new(format!(
      "expected {} generic argument(s) for `{name}`, got {}",
      generics.len(),
      args.len()
    )));
  }
  Ok(())
}

fn generic_bindings(generics: &[Arc<str>], args: &[Arc<CalcitTypeAnnotation>]) -> TypeBindings {
  generics.iter().cloned().zip(args.iter().cloned()).collect()
}

fn qualify_type_ref(name: &str, default_ns: &str) -> (Arc<str>, Arc<str>) {
  let stripped = name.trim_start_matches('\'').trim_start_matches(':');
  if let Some((ns, def)) = stripped.rsplit_once('/') {
    (Arc::from(ns), Arc::from(def))
  } else if program::has_def_code(default_ns, stripped) {
    (Arc::from(default_ns), Arc::from(stripped))
  } else if let Some(target_ns) = program::lookup_def_target_in_import(default_ns, stripped) {
    (target_ns, Arc::from(stripped))
  } else {
    (Arc::from(super::CORE_NS), Arc::from(stripped))
  }
}

fn infer_nominal_path(default_ns: &str, name: &str) -> Option<(Arc<str>, Arc<str>)> {
  if program::has_def_code(default_ns, name) {
    Some((Arc::from(default_ns), Arc::from(name)))
  } else if program::has_def_code(super::CORE_NS, name) {
    Some((Arc::from(super::CORE_NS), Arc::from(name)))
  } else {
    None
  }
}

fn nominal_key(
  kind: &str,
  name: &EdnTag,
  args: &[Arc<CalcitTypeAnnotation>],
  path: Option<&(Arc<str>, Arc<str>)>,
  owner_ns: &str,
) -> String {
  let identity = path
    .map(|(ns, def)| format!("{ns}/{def}"))
    .unwrap_or_else(|| format!("{owner_ns}/{}", name.ref_str()));
  let rendered_args = args.iter().map(|arg| arg.to_brief_string()).collect::<Vec<_>>().join(",");
  format!("{kind}:{identity}<{rendered_args}>")
}

fn unsupported_type(reason: &str) -> DataShapeError {
  DataShapeError::new(format!("cannot derive a closed data shape: {reason}"))
}

fn shape_fingerprint(root: usize, nodes: &[DataShapeNode]) -> String {
  let mut hasher = Md5::new();
  hasher.update(format!("calcit-data-shape-v{DATA_SHAPE_ABI_VERSION};root={root};").as_bytes());
  for (node_id, node) in nodes.iter().enumerate() {
    hasher.update(format!("#{node_id}:").as_bytes());
    match node {
      DataShapeNode::Dynamic => hasher.update(b"dynamic;"),
      DataShapeNode::Nil => hasher.update(b"nil;"),
      DataShapeNode::Unit => hasher.update(b"unit;"),
      DataShapeNode::Bool => hasher.update(b"bool;"),
      DataShapeNode::Number => hasher.update(b"number;"),
      DataShapeNode::String => hasher.update(b"string;"),
      DataShapeNode::Symbol => hasher.update(b"symbol;"),
      DataShapeNode::Tag => hasher.update(b"tag;"),
      DataShapeNode::Buffer => hasher.update(b"buffer;"),
      DataShapeNode::CirruQuote => hasher.update(b"cirru-quote;"),
      DataShapeNode::Optional(inner) => hasher.update(format!("optional:{inner};").as_bytes()),
      DataShapeNode::MapOption {
        nominal,
        nominal_path,
        inner,
      } => {
        update_nominal_fingerprint(&mut hasher, "map-option", nominal.name().ref_str(), nominal_path.as_ref(), &[]);
        hasher.update(format!("inner:{inner};").as_bytes());
      }
      DataShapeNode::List(inner) => hasher.update(format!("list:{inner};").as_bytes()),
      DataShapeNode::Set(inner) => hasher.update(format!("set:{inner};").as_bytes()),
      DataShapeNode::Map { key, value } => hasher.update(format!("map:{key}:{value};").as_bytes()),
      DataShapeNode::Ref(inner) => hasher.update(format!("ref:{inner};").as_bytes()),
      DataShapeNode::Struct {
        nominal,
        nominal_path,
        type_args,
        fields,
      } => {
        update_nominal_fingerprint(&mut hasher, "struct", nominal.name.ref_str(), nominal_path.as_ref(), type_args);
        for (field, child) in fields {
          hasher.update(format!("field:{}:{child};", field.ref_str()).as_bytes());
        }
      }
      DataShapeNode::Enum {
        nominal,
        nominal_path,
        type_args,
        variants,
      } => {
        update_nominal_fingerprint(&mut hasher, "enum", nominal.name().ref_str(), nominal_path.as_ref(), type_args);
        for (variant, payloads) in variants {
          hasher.update(format!("variant:{}:", variant.ref_str()).as_bytes());
          for child in payloads {
            hasher.update(format!("{child},").as_bytes());
          }
          hasher.update(b";");
        }
      }
    }
  }
  hex::encode(hasher.finalize())
}

fn update_nominal_fingerprint(
  hasher: &mut Md5,
  kind: &str,
  fallback_name: &str,
  nominal_path: Option<&(Arc<str>, Arc<str>)>,
  type_args: &[Arc<CalcitTypeAnnotation>],
) {
  let identity = nominal_path
    .map(|(ns, def)| format!("{ns}/{def}"))
    .unwrap_or_else(|| format!("?/{fallback_name}"));
  hasher.update(format!("{kind}:{identity}<").as_bytes());
  for arg in type_args {
    hasher.update(arg.to_brief_string().as_bytes());
    hasher.update(b",");
  }
  hasher.update(b">;");
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitEnumValue, CalcitGenericBound, CalcitImpl, CalcitList, CalcitStructValue, CalcitTrait};

  fn phantom_box() -> Arc<CalcitStructDef> {
    Arc::new(CalcitStructDef {
      name: EdnTag::new("PhantomBox"),
      fields: Arc::new(vec![]),
      field_types: Arc::new(vec![]),
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    })
  }

  #[test]
  fn fingerprint_includes_nominal_type_arguments_even_when_fields_do_not() {
    let nominal = phantom_box();
    let number = DataShapeGraph::build(
      &CalcitTypeAnnotation::Struct(nominal.clone(), Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)])),
      "tests.shape",
    )
    .expect("number shape");
    let string = DataShapeGraph::build(
      &CalcitTypeAnnotation::Struct(nominal, Arc::new(vec![Arc::new(CalcitTypeAnnotation::String)])),
      "tests.shape",
    )
    .expect("string shape");

    assert_ne!(number.fingerprint(), string.fingerprint());
  }

  #[test]
  fn rejects_invalid_child_references() {
    let error = DataShapeGraph::from_nodes(0, vec![DataShapeNode::List(1)]).expect_err("missing child must fail");
    assert!(error.to_string().contains("missing child #1"));
  }

  #[test]
  fn rejects_dynamic_and_incomplete_generic_types() {
    let dynamic = DataShapeGraph::build(&CalcitTypeAnnotation::Dynamic, "tests.shape").expect_err("dynamic must fail");
    assert!(dynamic.to_string().contains("Dynamic is forbidden"));

    let generic = Arc::new(CalcitStructDef {
      name: EdnTag::new("Box"),
      fields: Arc::new(vec![EdnTag::new("value")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))]),
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(Vec::<CalcitGenericBound>::new()),
      impls: vec![],
    });
    let incomplete = DataShapeGraph::build(&CalcitTypeAnnotation::Struct(generic, Arc::new(vec![])), "tests.shape")
      .expect_err("missing generic argument must fail");
    assert!(incomplete.to_string().contains("expected 1 generic argument"));
  }

  #[test]
  fn rejects_recursive_type_slots() {
    let name: Arc<str> = Arc::from("data-shape-recursive-slot");
    super::super::push_type_slot_override(name.clone(), Arc::new(CalcitTypeAnnotation::TypeSlot(name.clone())));
    let error = DataShapeGraph::build(&CalcitTypeAnnotation::TypeSlot(name.clone()), "tests.shape")
      .expect_err("recursive slot must fail without overflowing the stack");
    super::super::pop_type_slot_override(&name);

    assert!(error.to_string().contains("recursive type slot"), "unexpected error: {error}");
  }

  #[test]
  fn rejects_structurally_equal_but_distinct_nominal_declarations() {
    let nominal = Arc::new(CalcitStructDef::from_fields(EdnTag::new("Point"), vec![]));
    let shape =
      DataShapeGraph::build(&CalcitTypeAnnotation::Struct(nominal.clone(), Arc::new(vec![])), "tests.shape").expect("point shape");
    let impostor = Arc::new((*nominal).clone());
    let struct_value = Calcit::Struct(CalcitStructValue {
      struct_ref: impostor,
      values: Arc::new(vec![]),
    });

    let error = shape
      .validate_value(&struct_value)
      .expect_err("distinct struct declaration must fail");
    assert!(error.message.contains("expected nominal struct"), "unexpected error: {error}");

    let enum_prototype = || CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(EdnTag::new("Outcome"), vec![EdnTag::new("none")])),
      values: Arc::new(vec![Calcit::List(Arc::new(CalcitList::default()))]),
    };
    let nominal = Arc::new(CalcitEnumDef::from_struct(enum_prototype()).expect("outcome enum"));
    let shape = DataShapeGraph::build(&CalcitTypeAnnotation::Enum(nominal, Arc::new(vec![])), "tests.shape").expect("outcome shape");
    let impostor = Arc::new(CalcitEnumDef::from_struct(enum_prototype()).expect("impostor outcome enum"));
    let enum_value = Calcit::Enum(CalcitEnumValue {
      tag: Arc::new(Calcit::Tag(EdnTag::new("none"))),
      extra: vec![],
      sum_type: Some(impostor),
    });

    let error = shape.validate_value(&enum_value).expect_err("distinct enum declaration must fail");
    assert!(error.message.contains("expected nominal enum"), "unexpected error: {error}");
  }

  #[test]
  fn enforces_generic_where_bounds_while_deriving() {
    let marker = Arc::new(CalcitTrait::new(EdnTag::new("DataMarker"), vec![], vec![]));
    let bounded = Arc::new(CalcitStructDef {
      name: EdnTag::new("MarkedBox"),
      fields: Arc::new(vec![EdnTag::new("value")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))]),
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![CalcitGenericBound {
        name: Arc::from("T"),
        traits: Arc::new(vec![marker.clone()]),
      }]),
      impls: vec![],
    });

    let rejected = DataShapeGraph::build(
      &CalcitTypeAnnotation::Struct(bounded.clone(), Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)])),
      "tests.shape",
    )
    .expect_err("Number does not implement the nominal marker trait");
    assert!(rejected.to_string().contains("does not satisfy"));

    let mut marked_value = CalcitStructDef::from_fields(EdnTag::new("MarkedValue"), vec![]);
    marked_value.impls.push(Arc::new(CalcitImpl {
      name: EdnTag::new("MarkedValueDataMarker"),
      origin: Some(marker),
      fields: Arc::new(vec![]),
      values: Arc::new(vec![]),
    }));
    DataShapeGraph::build(
      &CalcitTypeAnnotation::Struct(
        bounded,
        Arc::new(vec![Arc::new(CalcitTypeAnnotation::Struct(
          Arc::new(marked_value),
          Arc::new(vec![]),
        ))]),
      ),
      "tests.shape",
    )
    .expect("nominal marker implementation should satisfy the bound");
  }
}
