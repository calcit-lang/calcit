use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use cirru_edn::{Edn, EdnRecordView, EdnTag, EdnTupleView};

use crate::builtins::quick_build_atom;
use crate::calcit::type_annotation::{TypeBindings, validate_runtime_generic_where_bounds};
use crate::calcit::{self, Calcit, CalcitEnum, CalcitList, CalcitRecord, CalcitStruct, CalcitTuple, CalcitTypeAnnotation};
use crate::program;

const MAX_DECODE_DEPTH: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EdnDecoderGraph {
  pub(crate) root: usize,
  pub(crate) nodes: Vec<EdnDecodeNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EdnDecodeNode {
  Unit,
  Bool,
  Number,
  String,
  Symbol,
  Tag,
  Buffer,
  CirruQuote,
  Optional(usize),
  List(usize),
  Set(usize),
  Map {
    key: usize,
    value: usize,
  },
  Ref(usize),
  Struct {
    nominal: Arc<CalcitStruct>,
    nominal_path: Option<(Arc<str>, Arc<str>)>,
    fields: Vec<(EdnTag, usize)>,
  },
  Enum {
    nominal: Arc<CalcitEnum>,
    nominal_path: Option<(Arc<str>, Arc<str>)>,
    variants: Vec<(EdnTag, Vec<usize>)>,
  },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EdnDecodeTypeError {
  message: String,
}

impl EdnDecodeTypeError {
  fn new(message: impl Into<String>) -> Self {
    Self { message: message.into() }
  }
}

impl fmt::Display for EdnDecodeTypeError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.message)
  }
}

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

struct GraphBuilder {
  nodes: Vec<Option<EdnDecodeNode>>,
  nominal_nodes: HashMap<String, usize>,
  resolving_aliases: HashSet<String>,
  resolving_slots: HashSet<String>,
}

impl EdnDecoderGraph {
  pub(crate) fn build(target: &CalcitTypeAnnotation, default_ns: &str) -> Result<Self, EdnDecodeTypeError> {
    let mut builder = GraphBuilder {
      nodes: vec![],
      nominal_nodes: HashMap::new(),
      resolving_aliases: HashSet::new(),
      resolving_slots: HashSet::new(),
    };
    let root = builder.build_type(target, default_ns)?;
    let nodes = builder
      .nodes
      .into_iter()
      .enumerate()
      .map(|(idx, node)| node.ok_or_else(|| EdnDecodeTypeError::new(format!("decoder graph node #{idx} was not completed"))))
      .collect::<Result<Vec<_>, _>>()?;
    Ok(Self { root, nodes })
  }

  pub(crate) fn decode(&self, input: &Edn) -> Result<Calcit, EdnDecodeError> {
    self.decode_node(self.root, input, "$", 0)
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
        EdnDecodeNode::Struct { nominal_path, .. } | EdnDecodeNode::Enum { nominal_path, .. } => nominal_path,
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

  fn decode_node(&self, node_id: usize, input: &Edn, path: &str, depth: usize) -> Result<Calcit, EdnDecodeError> {
    if depth > MAX_DECODE_DEPTH {
      return Err(EdnDecodeError::at(path, format!("decode nesting exceeds {MAX_DECODE_DEPTH}")));
    }
    let Some(node) = self.nodes.get(node_id) else {
      return Err(EdnDecodeError::at(path, format!("invalid decoder graph node #{node_id}")));
    };

    match node {
      EdnDecodeNode::Unit => match input {
        Edn::Nil => Ok(Calcit::Nil),
        _ => Err(kind_mismatch(path, "nil", input)),
      },
      EdnDecodeNode::Bool => match input {
        Edn::Bool(value) => Ok(Calcit::Bool(*value)),
        _ => Err(kind_mismatch(path, "bool", input)),
      },
      EdnDecodeNode::Number => match input {
        Edn::Number(value) => Ok(Calcit::Number(*value)),
        _ => Err(kind_mismatch(path, "number", input)),
      },
      EdnDecodeNode::String => match input {
        Edn::Str(value) => Ok(Calcit::Str(value.clone())),
        _ => Err(kind_mismatch(path, "string", input)),
      },
      EdnDecodeNode::Symbol => match input {
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
      EdnDecodeNode::Tag => match input {
        Edn::Tag(value) => Ok(Calcit::Tag(value.clone())),
        _ => Err(kind_mismatch(path, "tag", input)),
      },
      EdnDecodeNode::Buffer => match input {
        Edn::Buffer(value) => Ok(Calcit::Buffer(value.clone())),
        _ => Err(kind_mismatch(path, "buffer", input)),
      },
      EdnDecodeNode::CirruQuote => match input {
        Edn::Quote(value) => Ok(Calcit::CirruQuote(value.clone())),
        _ => Err(kind_mismatch(path, "cirru-quote", input)),
      },
      EdnDecodeNode::Optional(inner) => {
        if matches!(input, Edn::Nil) {
          Ok(Calcit::Nil)
        } else {
          self.decode_node(*inner, input, path, depth + 1)
        }
      }
      EdnDecodeNode::List(inner) => match input {
        Edn::List(items) => {
          let mut values = Vec::with_capacity(items.0.len());
          for (idx, item) in items.0.iter().enumerate() {
            values.push(self.decode_node(*inner, item, &format!("{path}[{idx}]"), depth + 1)?);
          }
          Ok(Calcit::List(Arc::new(CalcitList::Vector(values))))
        }
        _ => Err(kind_mismatch(path, "list", input)),
      },
      EdnDecodeNode::Set(inner) => match input {
        Edn::Set(items) => {
          let mut values = rpds::HashTrieSet::new_sync();
          for item in items.0.iter() {
            values.insert_mut(self.decode_node(*inner, item, &format!("{path}.item"), depth + 1)?);
          }
          Ok(Calcit::Set(values))
        }
        _ => Err(kind_mismatch(path, "set", input)),
      },
      EdnDecodeNode::Map { key, value } => match input {
        Edn::Map(entries) => {
          let mut values = rpds::HashTrieMap::new_sync();
          for (raw_key, raw_value) in entries.0.iter() {
            let decoded_key = self.decode_node(*key, raw_key, &format!("{path}.key"), depth + 1)?;
            let decoded_value = self.decode_node(*value, raw_value, &format!("{path}.value"), depth + 1)?;
            values.insert_mut(decoded_key, decoded_value);
          }
          Ok(Calcit::Map(values))
        }
        _ => Err(kind_mismatch(path, "map", input)),
      },
      EdnDecodeNode::Ref(inner) => match input {
        Edn::Atom(value) => {
          let decoded = self.decode_node(*inner, value, &format!("{path}.value"), depth + 1)?;
          Ok(quick_build_atom(decoded))
        }
        _ => Err(kind_mismatch(path, "atom", input)),
      },
      EdnDecodeNode::Struct { nominal, fields, .. } => match input {
        Edn::Record(EdnRecordView { tag, pairs }) => {
          if tag != &nominal.name {
            return Err(EdnDecodeError::at(
              path,
              format!("expected record :{}, got record :{tag}", nominal.name),
            ));
          }

          let expected_names: HashSet<&str> = fields.iter().map(|(field, _)| field.ref_str()).collect();
          let actual_names: HashSet<&str> = pairs.iter().map(|(field, _)| field.ref_str()).collect();
          if actual_names.len() != pairs.len() {
            let duplicates = sorted_duplicate_names(pairs.iter().map(|(field, _)| field.ref_str()));
            return Err(EdnDecodeError::at(
              path,
              format!("record :{} has duplicate fields [{}]", nominal.name, duplicates.join(", ")),
            ));
          }
          if expected_names != actual_names {
            let missing = sorted_name_diff(&expected_names, &actual_names);
            let unknown = sorted_name_diff(&actual_names, &expected_names);
            return Err(EdnDecodeError::at(
              path,
              format!(
                "record :{} fields mismatch; missing [{}], unknown [{}]",
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
              .expect("record field sets were checked");
            values.push(self.decode_node(*field_node, raw_value, &format!("{path}.{}", field.ref_str()), depth + 1)?);
          }
          Ok(Calcit::Record(CalcitRecord {
            struct_ref: nominal.clone(),
            values: Arc::new(values),
          }))
        }
        _ => Err(kind_mismatch(path, &format!("record :{}", nominal.name), input)),
      },
      EdnDecodeNode::Enum { nominal, variants, .. } => match input {
        Edn::Tuple(EdnTupleView { tag, enum_tag, extra }) => {
          let Some(actual_enum_name) = enum_tag.as_deref().and_then(edn_name) else {
            return Err(EdnDecodeError::at(
              path,
              format!("expected enum :{}, got ordinary tuple", nominal.name()),
            ));
          };
          if actual_enum_name != nominal.name().ref_str() {
            return Err(EdnDecodeError::at(
              path,
              format!("expected enum :{}, got enum :{actual_enum_name}", nominal.name()),
            ));
          }
          let Some(actual_tag) = edn_name(tag) else {
            return Err(EdnDecodeError::at(path, format!("enum :{} variant must be a tag", nominal.name())));
          };
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
          Ok(Calcit::Tuple(CalcitTuple {
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

impl GraphBuilder {
  fn push(&mut self, node: EdnDecodeNode) -> usize {
    let id = self.nodes.len();
    self.nodes.push(Some(node));
    id
  }

  fn build_type(&mut self, target: &CalcitTypeAnnotation, default_ns: &str) -> Result<usize, EdnDecodeTypeError> {
    match target {
      CalcitTypeAnnotation::Unit => Ok(self.push(EdnDecodeNode::Unit)),
      CalcitTypeAnnotation::Bool => Ok(self.push(EdnDecodeNode::Bool)),
      CalcitTypeAnnotation::Number => Ok(self.push(EdnDecodeNode::Number)),
      CalcitTypeAnnotation::String => Ok(self.push(EdnDecodeNode::String)),
      CalcitTypeAnnotation::Symbol => Ok(self.push(EdnDecodeNode::Symbol)),
      CalcitTypeAnnotation::Tag => Ok(self.push(EdnDecodeNode::Tag)),
      CalcitTypeAnnotation::Buffer => Ok(self.push(EdnDecodeNode::Buffer)),
      CalcitTypeAnnotation::CirruQuote => Ok(self.push(EdnDecodeNode::CirruQuote)),
      CalcitTypeAnnotation::Optional(inner) => {
        let inner = self.build_type(inner, default_ns)?;
        Ok(self.push(EdnDecodeNode::Optional(inner)))
      }
      CalcitTypeAnnotation::List(inner) => {
        let inner = self.build_type(inner, default_ns)?;
        Ok(self.push(EdnDecodeNode::List(inner)))
      }
      CalcitTypeAnnotation::Set(inner) => {
        let inner = self.build_type(inner, default_ns)?;
        Ok(self.push(EdnDecodeNode::Set(inner)))
      }
      CalcitTypeAnnotation::Map(key, value) => {
        let key = self.build_type(key, default_ns)?;
        let value = self.build_type(value, default_ns)?;
        Ok(self.push(EdnDecodeNode::Map { key, value }))
      }
      CalcitTypeAnnotation::Ref(inner) => {
        let inner = self.build_type(inner, default_ns)?;
        Ok(self.push(EdnDecodeNode::Ref(inner)))
      }
      CalcitTypeAnnotation::Struct(nominal, args) => {
        let path = infer_nominal_path(default_ns, nominal.name.ref_str());
        self.build_struct(nominal.clone(), args, path, default_ns)
      }
      CalcitTypeAnnotation::Record(nominal) => {
        let path = infer_nominal_path(default_ns, nominal.name.ref_str());
        self.build_struct(nominal.clone(), &Arc::new(vec![]), path, default_ns)
      }
      CalcitTypeAnnotation::Enum(nominal, args) => {
        let path = infer_nominal_path(default_ns, nominal.name().ref_str());
        self.build_enum(nominal.clone(), args, path, default_ns)
      }
      CalcitTypeAnnotation::Tuple(nominal) => {
        let path = infer_nominal_path(default_ns, nominal.name().ref_str());
        self.build_enum(nominal.clone(), &Arc::new(vec![]), path, default_ns)
      }
      CalcitTypeAnnotation::TypeRef(name, args) => self.build_named(name, args, default_ns),
      CalcitTypeAnnotation::TypeSlot(name) => match calcit::resolve_type_slot(name) {
        Some(resolved) => {
          let slot_key = name.to_string();
          if !self.resolving_slots.insert(slot_key.clone()) {
            return Err(EdnDecodeTypeError::new(format!(
              "parse-cirru-edn-as found a recursive type slot at `*{name}`"
            )));
          }
          let result = self.build_type(&resolved, default_ns);
          self.resolving_slots.remove(&slot_key);
          result
        }
        None => Err(EdnDecodeTypeError::new(format!(
          "parse-cirru-edn-as cannot derive a decoder for unbound type slot `*{name}`"
        ))),
      },
      CalcitTypeAnnotation::Dynamic => Err(unsupported_type("Dynamic is forbidden in a strict decoder")),
      CalcitTypeAnnotation::TypeVar(name) => Err(unsupported_type(&format!("generic variable '{name} is not bound"))),
      CalcitTypeAnnotation::DynTuple => Err(unsupported_type("ordinary tuple has no declared enum schema")),
      CalcitTypeAnnotation::DynFn | CalcitTypeAnnotation::Fn(_) => Err(unsupported_type("function values are not EDN data")),
      CalcitTypeAnnotation::Trait(_) | CalcitTypeAnnotation::TraitSet(_) => {
        Err(unsupported_type("trait constraints are not serializable data types"))
      }
      CalcitTypeAnnotation::JsObject => Err(unsupported_type("JsObject is an opaque host value")),
      CalcitTypeAnnotation::Custom(value) => Err(unsupported_type(&format!("custom type `{value}` is not decodable"))),
      CalcitTypeAnnotation::Variadic(_) => Err(unsupported_type("Variadic is a function parameter constraint")),
    }
  }

  fn build_named(
    &mut self,
    name: &str,
    args: &Arc<Vec<Arc<CalcitTypeAnnotation>>>,
    default_ns: &str,
  ) -> Result<usize, EdnDecodeTypeError> {
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
        return Err(EdnDecodeTypeError::new(format!(
          "parse-cirru-edn-as cannot apply generic arguments to type alias `{ns}/{def}`"
        )));
      }
      let alias_key = format!("{ns}/{def}");
      if !self.resolving_aliases.insert(alias_key.clone()) {
        return Err(EdnDecodeTypeError::new(format!(
          "parse-cirru-edn-as found a recursive type alias at `{alias_key}`; recursive data must use a nominal struct or enum"
        )));
      }
      let result = self.build_type(schema.as_ref(), &ns);
      self.resolving_aliases.remove(&alias_key);
      return result;
    }

    Err(EdnDecodeTypeError::new(format!(
      "parse-cirru-edn-as cannot resolve named type `{ns}/{def}`"
    )))
  }

  fn build_struct(
    &mut self,
    nominal: Arc<CalcitStruct>,
    args: &Arc<Vec<Arc<CalcitTypeAnnotation>>>,
    nominal_path: Option<(Arc<str>, Arc<str>)>,
    default_ns: &str,
  ) -> Result<usize, EdnDecodeTypeError> {
    validate_generic_application(&nominal.name.to_string(), &nominal.generics, args)?;
    if nominal.fields.len() != nominal.field_types.len() {
      return Err(EdnDecodeTypeError::new(format!(
        "parse-cirru-edn-as cannot derive `{}`: {} fields but {} field types",
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
      .map_err(|message| EdnDecodeTypeError::new(format!("parse-cirru-edn-as cannot derive `{}`: {message}", nominal.name)))?;
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
    self.nodes[node_id] = Some(EdnDecodeNode::Struct {
      nominal,
      nominal_path,
      fields,
    });
    Ok(node_id)
  }

  fn build_enum(
    &mut self,
    nominal: Arc<CalcitEnum>,
    args: &Arc<Vec<Arc<CalcitTypeAnnotation>>>,
    nominal_path: Option<(Arc<str>, Arc<str>)>,
    default_ns: &str,
  ) -> Result<usize, EdnDecodeTypeError> {
    validate_generic_application(&nominal.name().to_string(), nominal.generics(), args)?;
    let owner_ns = nominal_path.as_ref().map(|(ns, _)| ns.as_ref()).unwrap_or(default_ns);
    for arg in args.iter() {
      self.build_type(arg, owner_ns)?;
    }
    let bindings = generic_bindings(nominal.generics(), args);
    validate_runtime_generic_where_bounds(&bindings, nominal.where_bounds())
      .map_err(|message| EdnDecodeTypeError::new(format!("parse-cirru-edn-as cannot derive `{}`: {message}", nominal.name())))?;
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
    self.nodes[node_id] = Some(EdnDecodeNode::Enum {
      nominal,
      nominal_path,
      variants,
    });
    Ok(node_id)
  }
}

fn validate_generic_application(
  name: &str,
  generics: &[Arc<str>],
  args: &[Arc<CalcitTypeAnnotation>],
) -> Result<(), EdnDecodeTypeError> {
  if generics.len() != args.len() {
    return Err(EdnDecodeTypeError::new(format!(
      "parse-cirru-edn-as expected {} generic argument(s) for `{name}`, got {}",
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
    (Arc::from(calcit::CORE_NS), Arc::from(stripped))
  }
}

fn infer_nominal_path(default_ns: &str, name: &str) -> Option<(Arc<str>, Arc<str>)> {
  if program::has_def_code(default_ns, name) {
    Some((Arc::from(default_ns), Arc::from(name)))
  } else if program::has_def_code(calcit::CORE_NS, name) {
    Some((Arc::from(calcit::CORE_NS), Arc::from(name)))
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

fn unsupported_type(reason: &str) -> EdnDecodeTypeError {
  EdnDecodeTypeError::new(format!("parse-cirru-edn-as cannot derive a strict decoder: {reason}"))
}

fn edn_name(value: &Edn) -> Option<&str> {
  match value {
    Edn::Tag(value) => Some(value.ref_str()),
    Edn::Symbol(value) | Edn::Str(value) => Some(value.as_ref()),
    _ => None,
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
    Edn::Tuple(tuple) if tuple.enum_tag.is_some() => "enum",
    Edn::Tuple(_) => "tuple",
    Edn::List(_) => "list",
    Edn::Set(_) => "set",
    Edn::Map(_) => "map",
    Edn::Record(_) => "record",
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
  use crate::calcit::{CalcitGenericBound, CalcitImpl, CalcitTrait, EnumVariant};

  fn person_struct() -> Arc<CalcitStruct> {
    Arc::new(CalcitStruct {
      name: EdnTag::new("Person"),
      fields: Arc::new(vec![EdnTag::new("age"), EdnTag::new("name")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    })
  }

  #[test]
  fn decodes_struct_fields_deeply_and_preserves_nominal_identity() {
    let person = person_struct();
    let graph = EdnDecoderGraph::build(&CalcitTypeAnnotation::Struct(person.clone(), Arc::new(vec![])), "tests.edn")
      .expect("derive person decoder");
    let input = cirru_edn::parse("%{} :Person (:age 23) (:name |Ada)").expect("parse edn");
    let decoded = graph.decode(&input).expect("decode person");
    let Calcit::Record(record) = decoded else {
      panic!("expected record");
    };
    assert!(Arc::ptr_eq(&record.struct_ref, &person));
    assert_eq!(record.values.as_ref(), &[Calcit::Number(23.0), Calcit::Str(Arc::from("Ada"))]);
  }

  #[test]
  fn reports_nested_field_path() {
    let person = person_struct();
    let target = CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Struct(person, Arc::new(vec![]))));
    let graph = EdnDecoderGraph::build(&target, "tests.edn").expect("derive list decoder");
    let input = cirru_edn::parse("[] $ %{} :Person (:age |old) (:name |Ada)").expect("parse edn");
    let error = graph.decode(&input).expect_err("age must fail");
    assert_eq!(error.path, "$[0].age");
    assert!(error.message.contains("expected number, got string"));
  }

  #[test]
  fn rejects_duplicate_struct_fields() {
    let person = person_struct();
    let graph =
      EdnDecoderGraph::build(&CalcitTypeAnnotation::Struct(person, Arc::new(vec![])), "tests.edn").expect("derive person decoder");
    let input = cirru_edn::parse("%{} :Person (:age 23) (:age 24) (:name |Ada)").expect("parse duplicate fields");
    let error = graph.decode(&input).expect_err("duplicate fields must fail");
    assert_eq!(error.path, "$");
    assert!(error.message.contains("duplicate fields [age]"), "unexpected error: {error}");
  }

  #[test]
  fn rejects_dynamic_and_incomplete_generic_types() {
    let dynamic = EdnDecoderGraph::build(&CalcitTypeAnnotation::Dynamic, "tests.edn").expect_err("dynamic must fail");
    assert!(dynamic.to_string().contains("Dynamic is forbidden"));

    let generic = Arc::new(CalcitStruct {
      name: EdnTag::new("Box"),
      fields: Arc::new(vec![EdnTag::new("value")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))]),
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(Vec::<CalcitGenericBound>::new()),
      impls: vec![],
    });
    let incomplete = EdnDecoderGraph::build(&CalcitTypeAnnotation::Struct(generic, Arc::new(vec![])), "tests.edn")
      .expect_err("missing generic argument must fail");
    assert!(incomplete.to_string().contains("expected 1 generic argument"));
  }

  #[test]
  fn rejects_recursive_type_slots() {
    let name: Arc<str> = Arc::from("strict-edn-recursive-slot");
    calcit::push_type_slot_override(name.clone(), Arc::new(CalcitTypeAnnotation::TypeSlot(name.clone())));
    let error = EdnDecoderGraph::build(&CalcitTypeAnnotation::TypeSlot(name.clone()), "tests.edn")
      .expect_err("recursive slot must fail without overflowing the stack");
    calcit::pop_type_slot_override(&name);

    assert!(error.to_string().contains("recursive type slot"), "unexpected error: {error}");
  }

  #[test]
  fn enforces_generic_where_bounds_while_deriving() {
    let marker = Arc::new(CalcitTrait::new(EdnTag::new("EdnMarker"), vec![], vec![]));
    let bounded = Arc::new(CalcitStruct {
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

    let rejected = EdnDecoderGraph::build(
      &CalcitTypeAnnotation::Struct(bounded.clone(), Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)])),
      "tests.edn",
    )
    .expect_err("Number does not implement the nominal marker trait");
    assert!(rejected.to_string().contains("does not satisfy"));

    let mut marked_value = CalcitStruct::from_fields(EdnTag::new("MarkedValue"), vec![]);
    marked_value.impls.push(Arc::new(CalcitImpl {
      name: EdnTag::new("MarkedValueEdnMarker"),
      origin: Some(marker),
      fields: Arc::new(vec![]),
      values: Arc::new(vec![]),
    }));
    EdnDecoderGraph::build(
      &CalcitTypeAnnotation::Struct(
        bounded,
        Arc::new(vec![Arc::new(CalcitTypeAnnotation::Struct(
          Arc::new(marked_value),
          Arc::new(vec![]),
        ))]),
      ),
      "tests.edn",
    )
    .expect("nominal marker implementation should satisfy the bound");
  }

  #[test]
  fn decodes_enum_and_checks_payload_arity() {
    let prototype = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct {
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
    let enum_def = Arc::new(CalcitEnum::from_record(prototype).expect("enum prototype"));
    assert!(matches!(enum_def.variants(), [EnumVariant { .. }, EnumVariant { .. }]));
    let graph = EdnDecoderGraph::build(&CalcitTypeAnnotation::Enum(enum_def.clone(), Arc::new(vec![])), "tests.edn")
      .expect("derive enum decoder");
    let input = cirru_edn::parse("%:: :ResultText :err |oops").expect("parse edn");
    let decoded = graph.decode(&input).expect("decode enum");
    let Calcit::Tuple(tuple) = decoded else {
      panic!("expected tuple");
    };
    assert!(tuple.sum_type.as_ref().is_some_and(|actual| Arc::ptr_eq(actual, &enum_def)));

    let invalid = cirru_edn::parse("%:: :ResultText :ok 1").expect("parse invalid enum");
    let error = graph.decode(&invalid).expect_err("arity must fail");
    assert!(error.message.contains("expects 0 payload(s), got 1"));
  }
}
