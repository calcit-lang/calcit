use std::{
  cell::RefCell,
  cmp::Ordering,
  fmt,
  hash::{Hash, Hasher},
  sync::Arc,
};

use std::thread_local;

use cirru_edn::EdnTag;

use super::{Calcit, CalcitImport, CalcitList, CalcitProc, CalcitRecord, CalcitSyntax, CalcitTuple};
use crate::program;

thread_local! {
  static IMPORT_RESOLUTION_STACK: RefCell<Vec<(Arc<str>, Arc<str>)>> = const { RefCell::new(vec![]) };
}

/// Unified representation of type annotations propagated through preprocessing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalcitTypeAnnotation {
  Tag(EdnTag),
  Record(Arc<CalcitRecord>),
  Tuple(Arc<CalcitTuple>),
  Function(Arc<CalcitFnTypeAnnotation>),
  /// Hashset type
  Set,
  /// Variadic parameter type constraint (for & args)
  Variadic(Arc<CalcitTypeAnnotation>),
  /// Fallback for shapes that are not yet modeled explicitly in class Record
  Custom(Arc<Calcit>),
  /// No checking at static analaysis time
  Dynamic,
  /// Represents an type that can be nil or the given type
  Optional(Arc<CalcitTypeAnnotation>),
}

impl CalcitTypeAnnotation {
  /// Collect arg type hints for function parameters by scanning `assert-type` in body forms.
  ///
  /// This is intentionally different from return-type handling: return-type uses `hint-fn`, while
  /// arg types are sourced from `assert-type` inside function bodies. If no `assert-type` is found,
  /// the parameter stays `dynamic` and no checking occurs.
  pub fn collect_arg_type_hints_from_body(body_items: &[Calcit], params: &[Arc<str>]) -> Vec<Arc<CalcitTypeAnnotation>> {
    let dynamic = Arc::new(CalcitTypeAnnotation::Dynamic);
    let mut arg_types = vec![dynamic.clone(); params.len()];
    if params.is_empty() {
      return arg_types;
    }

    let mut param_index: std::collections::HashMap<Arc<str>, usize> = std::collections::HashMap::with_capacity(params.len());
    for (idx, sym) in params.iter().enumerate() {
      param_index.entry(sym.to_owned()).or_insert(idx);
    }

    for form in body_items {
      Self::scan_body_for_arg_types(form, &param_index, &mut arg_types);
    }

    let has_variadic = arg_types.iter().any(|ty| matches!(ty.as_ref(), CalcitTypeAnnotation::Variadic(_)));
    if has_variadic { arg_types } else { vec![dynamic; params.len()] }
  }

  /// Walk a form tree to find `(assert-type <param> <type>)` and map it to the param index.
  ///
  /// Unlike `parse_type_annotation_form`, this inspects raw body forms and ignores nested defn/defmacro.
  fn scan_body_for_arg_types(
    form: &Calcit,
    param_index: &std::collections::HashMap<Arc<str>, usize>,
    arg_types: &mut [Arc<CalcitTypeAnnotation>],
  ) {
    let list = match form {
      Calcit::List(xs) => xs,
      _ => return,
    };

    if let Some((target, type_expr)) = Self::extract_assert_type_args(list) {
      let sym = match target {
        Calcit::Symbol { sym, .. } => sym.to_owned(),
        Calcit::Local(local) => local.sym.to_owned(),
        _ => return,
      };

      if let Some(&idx) = param_index.get(&sym) {
        arg_types[idx] = CalcitTypeAnnotation::parse_type_annotation_form(type_expr);
      }
      return;
    }

    let head_is_nested_defn = matches!(
      list.first(),
      Some(Calcit::Syntax(CalcitSyntax::Defn, _)) | Some(Calcit::Syntax(CalcitSyntax::Defmacro, _))
    );
    if head_is_nested_defn {
      return;
    }

    for item in list.iter() {
      Self::scan_body_for_arg_types(item, param_index, arg_types);
    }
  }

  /// Extract `(assert-type target type-expr)` from a list.
  ///
  /// This differs from `preprocess_asset_type`: here we only read the raw AST to discover hints
  /// for function parameters, without mutating scopes or locals.
  fn extract_assert_type_args(list: &CalcitList) -> Option<(&Calcit, &Calcit)> {
    match list.first() {
      Some(Calcit::Syntax(CalcitSyntax::AssertType, _)) => {}
      Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "assert-type" => {}
      _ => return None,
    }

    let target = list.get(1)?;
    let type_expr = list.get(2)?;
    Some((target, type_expr))
  }

  pub fn parse_type_annotation_form(form: &Calcit) -> Arc<CalcitTypeAnnotation> {
    let is_optional_tag = |tag: &EdnTag| tag.ref_str().trim_start_matches(':') == "optional";

    if let Calcit::Tuple(tuple) = form {
      if let Calcit::Tag(tag) = tuple.tag.as_ref() {
        if is_optional_tag(tag) {
          if tuple.extra.len() != 1 {
            eprintln!("[Warn] :optional expects 1 argument, got {}", tuple.extra.len());
          }
          if let Some(inner_form) = tuple.extra.first() {
            let inner = Self::parse_type_annotation_form(inner_form);
            return Arc::new(CalcitTypeAnnotation::Optional(inner));
          }
        }
      }
    }

    if let Calcit::List(xs) = form {
      let is_tuple_constructor = match xs.first() {
        Some(Calcit::Proc(CalcitProc::NativeTuple)) => true,
        Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "::" => true,
        _ => false,
      };

      if is_tuple_constructor {
        // Check for variadic type: (:: :& :type)
        if xs.len() == 3 {
          if let (Some(Calcit::Tag(marker)), Some(inner_form)) = (xs.get(1), xs.get(2)) {
            if marker.ref_str().trim_start_matches(':') == "&" {
              let inner = Self::parse_type_annotation_form(inner_form);
              return Arc::new(CalcitTypeAnnotation::Variadic(inner));
            }
          }
        }

        // Check for optional: (:: :optional :type)
        if let Some(Calcit::Tag(tag)) = xs.get(1) {
          if is_optional_tag(tag) {
            if xs.len() != 3 {
              eprintln!("[Warn] :optional expects 1 argument, got {}", xs.len() as i64 - 2);
            }
            if let Some(inner_form) = xs.get(2) {
              let inner = Self::parse_type_annotation_form(inner_form);
              return Arc::new(CalcitTypeAnnotation::Optional(inner));
            }
          }
        }
      }
    }

    Arc::new(CalcitTypeAnnotation::from_calcit(form))
  }

  /// Render a concise representation used in warnings or logs
  pub fn to_brief_string(&self) -> String {
    match self {
      Self::Tag(tag) => format!(":{}", tag.ref_str()),
      Self::Record(record) => format!("record {}", record.name),
      Self::Tuple(_) => "tuple".to_string(),
      Self::Function(signature) => signature.render_signature_brief(),
      Self::Set => "set".to_string(),
      Self::Variadic(inner) => format!("variadic {}", inner.to_brief_string()),
      Self::Custom(inner) => format!("{inner}"),
      Self::Optional(inner) => format!("optional {}", inner.to_brief_string()),
      Self::Dynamic => "dynamic".to_string(),
    }
  }

  pub fn matches_annotation(&self, expected: &CalcitTypeAnnotation) -> bool {
    match (self, expected) {
      (_, Self::Dynamic) | (Self::Dynamic, _) => true,
      (_, Self::Optional(expected_inner)) => match self {
        Self::Optional(actual_inner) => actual_inner.matches_annotation(expected_inner),
        Self::Tag(tag) if tag.ref_str().trim_start_matches(':') == "nil" => true,
        _ => self.matches_annotation(expected_inner),
      },
      (Self::Optional(_), _) => false,
      (Self::Tag(a), Self::Tag(b)) => a.ref_str() == b.ref_str(),
      (Self::Record(a), Self::Record(b)) => a.name == b.name,
      (Self::Tuple(a), Self::Tuple(b)) => a.as_ref() == b.as_ref(),
      (Self::Function(a), Self::Function(b)) => a.matches_signature(b.as_ref()),
      (Self::Set, Self::Set) => true,
      (Self::Variadic(a), Self::Variadic(b)) => a.matches_annotation(b),
      (Self::Custom(a), Self::Custom(b)) => a.as_ref() == b.as_ref(),
      _ => false,
    }
  }

  pub fn from_calcit(value: &Calcit) -> Self {
    match value {
      Calcit::Nil => Self::from_tag_name("nil"),
      Calcit::Bool(_) => Self::from_tag_name("bool"),
      Calcit::Number(_) => Self::from_tag_name("number"),
      Calcit::Str(_) => Self::from_tag_name("string"),
      Calcit::Tag(tag) => {
        let tag_name = tag.ref_str().trim_start_matches(':');
        if tag_name == "any" || tag_name == "dynamic" {
          Self::Dynamic
        } else if tag_name == "set" {
          Self::Set
        } else {
          Self::Tag(tag.to_owned())
        }
      }
      Calcit::List(_) => Self::from_tag_name("list"),
      Calcit::Map(_) => Self::from_tag_name("map"),
      Calcit::Set(_) => Self::Set,
      Calcit::Record(record) => Self::Record(Arc::new(record.to_owned())),
      Calcit::Tuple(tuple) => {
        // Check for special tuple patterns
        if let Calcit::Tag(tag) = tuple.tag.as_ref() {
          let tag_name = tag.ref_str().trim_start_matches(':');
          if tag_name == "&" && tuple.extra.len() == 1 {
            // Variadic type: (& :type)
            return Self::Variadic(Arc::new(Self::from_calcit(&tuple.extra[0])));
          } else if tag_name == "optional" && tuple.extra.len() == 1 {
            // Optional type: (optional :type)
            return Self::Optional(Arc::new(Self::from_calcit(&tuple.extra[0])));
          }
        }
        Self::Tuple(Arc::new(tuple.to_owned()))
      }
      Calcit::Fn { info, .. } => Self::from_function_parts(info.arg_types.clone(), info.return_type.clone()),
      Calcit::Import(import) => Self::from_import(import).unwrap_or(Self::Dynamic),
      Calcit::Proc(proc) => {
        if let Some(signature) = proc.get_type_signature() {
          Self::from_function_parts(signature.arg_types, signature.return_type)
        } else {
          Self::Dynamic
        }
      }
      Calcit::Ref(_, _) => Self::from_tag_name("ref"),
      Calcit::Symbol { .. } => Self::from_tag_name("symbol"),
      Calcit::Buffer(_) => Self::from_tag_name("buffer"),
      Calcit::CirruQuote(_) => Self::from_tag_name("cirru-quote"),
      other => Self::Custom(Arc::new(other.to_owned())),
    }
  }

  pub fn from_tag_name(name: &str) -> Self {
    Self::Tag(EdnTag::from(name))
  }

  pub fn from_function_parts(arg_types: Vec<Arc<CalcitTypeAnnotation>>, return_type: Arc<CalcitTypeAnnotation>) -> Self {
    Self::Function(Arc::new(CalcitFnTypeAnnotation { arg_types, return_type }))
  }

  fn from_import(import: &CalcitImport) -> Option<Self> {
    let mut short_circuit = false;
    let mut pushed = false;

    IMPORT_RESOLUTION_STACK.with(|stack| {
      let mut stack = stack.borrow_mut();
      if stack
        .iter()
        .any(|(ns, def)| ns.as_ref() == import.ns.as_ref() && def.as_ref() == import.def.as_ref())
      {
        short_circuit = true;
      } else {
        stack.push((import.ns.clone(), import.def.clone()));
        pushed = true;
      }
    });

    if short_circuit {
      return None;
    }

    let resolved = program::lookup_evaled_def(import.ns.as_ref(), import.def.as_ref())
      .or_else(|| program::lookup_def_code(import.ns.as_ref(), import.def.as_ref()))
      .map(|value| CalcitTypeAnnotation::from_calcit(&value));

    if pushed {
      IMPORT_RESOLUTION_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        let _ = stack.pop();
      });
    }

    resolved
  }

  pub fn to_calcit(&self) -> Calcit {
    match self {
      Self::Tag(tag) => Calcit::Tag(tag.to_owned()),
      Self::Record(record) => Calcit::Record((**record).clone()),
      Self::Tuple(tuple) => Calcit::Tuple((**tuple).clone()),
      Self::Function(_) => Calcit::Tag(EdnTag::from("fn")),
      Self::Set => Calcit::Tag(EdnTag::from("set")),
      Self::Variadic(inner) => Calcit::Tuple(CalcitTuple {
        tag: Arc::new(Calcit::Tag(EdnTag::from("&"))),
        extra: vec![inner.to_calcit()],
        class: None,
        sum_type: None,
      }),
      Self::Custom(value) => value.as_ref().to_owned(),
      Self::Optional(inner) => Calcit::Tuple(CalcitTuple {
        tag: Arc::new(Calcit::Tag(EdnTag::from("optional"))),
        extra: vec![inner.to_calcit()],
        class: None,
        sum_type: None,
      }),
      Self::Dynamic => Calcit::Nil,
    }
  }

  pub fn as_record(&self) -> Option<&CalcitRecord> {
    match self {
      Self::Record(record) => Some(record),
      Self::Custom(value) => match value.as_ref() {
        Calcit::Record(record) => Some(record),
        _ => None,
      },
      Self::Optional(inner) => inner.as_record(),
      _ => None,
    }
  }

  pub fn as_tuple(&self) -> Option<&CalcitTuple> {
    match self {
      Self::Tuple(tuple) => Some(tuple),
      Self::Custom(value) => match value.as_ref() {
        Calcit::Tuple(tuple) => Some(tuple),
        _ => None,
      },
      Self::Optional(inner) => inner.as_tuple(),
      _ => None,
    }
  }

  pub fn as_tag(&self) -> Option<&EdnTag> {
    match self {
      Self::Tag(tag) => Some(tag),
      Self::Custom(value) => match value.as_ref() {
        Calcit::Tag(tag) => Some(tag),
        _ => None,
      },
      Self::Optional(inner) => inner.as_tag(),
      _ => None,
    }
  }

  pub fn as_function(&self) -> Option<&CalcitFnTypeAnnotation> {
    match self {
      Self::Function(signature) => Some(signature.as_ref()),
      Self::Optional(inner) => inner.as_function(),
      _ => None,
    }
  }

  pub fn describe(&self) -> String {
    match self {
      Self::Tag(tag) => format!("{} type", tag.ref_str().trim_start_matches(':')),
      Self::Record(record) => format!("record {}", record.name),
      Self::Tuple(tuple) => format!("tuple {:?}", tuple.tag),
      Self::Function(signature) => signature.describe(),
      Self::Set => "set type".to_string(),
      Self::Variadic(inner) => format!("variadic {}", inner.describe()),
      Self::Custom(_) => "custom type".to_string(),
      Self::Optional(inner) => format!("optional {}", inner.describe()),
      Self::Dynamic => "dynamic type".to_string(),
    }
  }

  fn variant_order(&self) -> u8 {
    match self {
      Self::Tag(_) => 0,
      Self::Record(_) => 1,
      Self::Tuple(_) => 2,
      Self::Function(_) => 3,
      Self::Set => 4,
      Self::Variadic(_) => 5,
      Self::Custom(_) => 6,
      Self::Optional(_) => 7,
      Self::Dynamic => 8,
    }
  }
}

impl fmt::Display for CalcitTypeAnnotation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.describe())
  }
}

impl Hash for CalcitTypeAnnotation {
  fn hash<H: Hasher>(&self, state: &mut H) {
    match self {
      Self::Tag(tag) => {
        "tag".hash(state);
        tag.hash(state);
      }
      Self::Record(record) => {
        "record".hash(state);
        let record = record.as_ref();
        record.name.hash(state);
        record.fields.hash(state);
        record.values.hash(state);
      }
      Self::Tuple(tuple) => {
        "tuple".hash(state);
        let tuple = tuple.as_ref();
        tuple.tag.hash(state);
        tuple.extra.hash(state);
      }
      Self::Function(signature) => {
        "function".hash(state);
        signature.arg_types.hash(state);
        signature.return_type.hash(state);
      }
      Self::Set => {
        "set".hash(state);
      }
      Self::Variadic(inner) => {
        "variadic".hash(state);
        inner.hash(state);
      }
      Self::Custom(value) => {
        "custom".hash(state);
        value.hash(state);
      }
      Self::Optional(inner) => {
        "optional".hash(state);
        inner.hash(state);
      }
      Self::Dynamic => {
        "dynamic".hash(state);
      }
    }
  }
}

impl PartialOrd for CalcitTypeAnnotation {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for CalcitTypeAnnotation {
  fn cmp(&self, other: &Self) -> Ordering {
    let kind_cmp = self.variant_order().cmp(&other.variant_order());
    if kind_cmp != Ordering::Equal {
      return kind_cmp;
    }

    match (self, other) {
      (Self::Tag(a), Self::Tag(b)) => a.cmp(b),
      (Self::Record(a), Self::Record(b)) => {
        let a = a.as_ref();
        let b = b.as_ref();
        a.name
          .cmp(&b.name)
          .then_with(|| a.fields.cmp(&b.fields))
          .then_with(|| a.values.cmp(&b.values))
      }
      (Self::Tuple(a), Self::Tuple(b)) => {
        let a = a.as_ref();
        let b = b.as_ref();
        a.tag.cmp(&b.tag).then_with(|| a.extra.cmp(&b.extra))
      }
      (Self::Function(a), Self::Function(b)) => a.arg_types.cmp(&b.arg_types).then_with(|| a.return_type.cmp(&b.return_type)),
      (Self::Set, Self::Set) => Ordering::Equal,
      (Self::Variadic(a), Self::Variadic(b)) => a.cmp(b),
      (Self::Custom(a), Self::Custom(b)) => a.cmp(b),
      (Self::Optional(a), Self::Optional(b)) => a.cmp(b),
      (Self::Dynamic, Self::Dynamic) => Ordering::Equal,
      _ => Ordering::Equal, // other variants already separated by kind order
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalcitFnTypeAnnotation {
  pub arg_types: Vec<Arc<CalcitTypeAnnotation>>,
  pub return_type: Arc<CalcitTypeAnnotation>,
}

impl CalcitFnTypeAnnotation {
  pub fn describe(&self) -> String {
    let args = if self.arg_types.is_empty() {
      "()".to_string()
    } else {
      let rendered = self.arg_types.iter().map(|t| t.describe()).collect::<Vec<_>>().join(", ");
      format!("({rendered})")
    };
    format!("fn{args} -> {}", self.return_type.describe())
  }

  pub fn render_signature_brief(&self) -> String {
    let args_repr = if self.arg_types.is_empty() {
      "()".to_string()
    } else {
      let parts = self.arg_types.iter().map(|t| t.to_brief_string()).collect::<Vec<_>>().join(", ");
      format!("({parts})")
    };

    format!("fn{args_repr} -> {}", self.return_type.to_brief_string())
  }

  pub fn matches_signature(&self, other: &CalcitFnTypeAnnotation) -> bool {
    if self.arg_types.len() != other.arg_types.len() {
      return false;
    }

    for (lhs, rhs) in self.arg_types.iter().zip(other.arg_types.iter()) {
      if !lhs.matches_annotation(rhs) {
        return false;
      }
    }

    self.return_type.matches_annotation(other.return_type.as_ref())
  }
}
