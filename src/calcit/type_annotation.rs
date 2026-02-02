use std::{
  cell::RefCell,
  cmp::Ordering,
  fmt,
  hash::{Hash, Hasher},
  sync::Arc,
};

use std::thread_local;

use cirru_edn::EdnTag;

use super::{Calcit, CalcitEnum, CalcitImport, CalcitList, CalcitProc, CalcitRecord, CalcitStruct, CalcitSyntax, CalcitTuple};
use crate::program;

thread_local! {
  static IMPORT_RESOLUTION_STACK: RefCell<Vec<(Arc<str>, Arc<str>)>> = const { RefCell::new(vec![]) };
}

/// Unified representation of type annotations propagated through preprocessing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalcitTypeAnnotation {
  Bool,
  Number,
  String,
  Symbol,
  Tag,
  /// List type with element type annotation
  /// `List Dynamic` for dynamic list, `List<T>` for typed list
  List(Arc<CalcitTypeAnnotation>),
  /// Map type with key and value type annotations
  Map(Arc<CalcitTypeAnnotation>, Arc<CalcitTypeAnnotation>),
  Record(Arc<CalcitRecord>),
  Tuple(Arc<CalcitTuple>),
  /// Any tuple type (when specific structure is not known)
  DynTuple,
  /// function type without a known signature
  DynFn,
  Fn(Arc<CalcitFnTypeAnnotation>),
  /// Hashset type
  Set(Arc<CalcitTypeAnnotation>),
  Ref(Arc<CalcitTypeAnnotation>),
  Buffer,
  CirruQuote,
  /// Variadic parameter type constraint (for & args)
  Variadic(Arc<CalcitTypeAnnotation>),
  /// Fallback for shapes that are not yet modeled explicitly in class Record
  Custom(Arc<Calcit>),
  /// No checking at static analaysis time
  Dynamic,
  /// Represents an type that can be nil or the given type
  Optional(Arc<CalcitTypeAnnotation>),
  /// Reference to an existing type definition (struct or enum)
  /// Used for struct field types that reference other types
  TypeRef {
    ns: Arc<str>,
    name: Arc<str>,
  },
}

impl CalcitTypeAnnotation {
  fn builtin_type_from_tag_name(name: &str) -> Option<Self> {
    match name {
      "nil" => Some(Self::Dynamic),
      "bool" => Some(Self::Bool),
      "number" => Some(Self::Number),
      "string" => Some(Self::String),
      "symbol" => Some(Self::Symbol),
      "tag" => Some(Self::Tag),
      "list" => Some(Self::List(Arc::new(Self::Dynamic))),
      "map" => Some(Self::Map(Arc::new(Self::Dynamic), Arc::new(Self::Dynamic))),
      "set" => Some(Self::Set(Arc::new(Self::Dynamic))),
      "tuple" => Some(Self::DynTuple),
      "fn" => Some(Self::DynFn),
      "ref" => Some(Self::Ref(Arc::new(Self::Dynamic))),
      "buffer" => Some(Self::Buffer),
      "cirru-quote" => Some(Self::CirruQuote),
      _ => None,
    }
  }

  fn builtin_tag_name(&self) -> Option<&'static str> {
    match self {
      Self::Bool => Some("bool"),
      Self::Number => Some("number"),
      Self::String => Some("string"),
      Self::Symbol => Some("symbol"),
      Self::Tag => Some("tag"),
      Self::List(_) => Some("list"),
      Self::Map(_, _) => Some("map"),
      Self::DynFn => Some("fn"),
      Self::Set(_) => Some("set"),
      Self::DynTuple => Some("tuple"),
      Self::Ref(_) => Some("ref"),
      Self::Buffer => Some("buffer"),
      Self::CirruQuote => Some("cirru-quote"),
      _ => None,
    }
  }

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

    arg_types
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
    let is_list_tag = |tag: &EdnTag| tag.ref_str().trim_start_matches(':') == "list";
    let is_map_tag = |tag: &EdnTag| tag.ref_str().trim_start_matches(':') == "map";
    let is_set_tag = |tag: &EdnTag| tag.ref_str().trim_start_matches(':') == "set";
    let is_ref_tag = |tag: &EdnTag| tag.ref_str().trim_start_matches(':') == "ref";
    let is_typeref_tag = |tag: &EdnTag| tag.ref_str().trim_start_matches(':') == "typeref";
    let get_str_from_calcit = |x: &Calcit| match x {
      Calcit::Tag(t) => Some(Arc::from(t.ref_str())),
      Calcit::Str(s) => Some(s.to_owned()),
      Calcit::Symbol { sym, .. } => Some(sym.to_owned()),
      _ => None,
    };

    if matches!(form, Calcit::Nil) {
      return Arc::new(CalcitTypeAnnotation::Dynamic);
    }

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
        // Check for list type: (:list :type)
        if is_list_tag(tag) {
          if tuple.extra.len() > 1 {
            eprintln!("[Warn] :list expects at most 1 argument, got {}", tuple.extra.len());
          }
          if let Some(inner_form) = tuple.extra.first() {
            let inner = Self::parse_type_annotation_form(inner_form);
            return Arc::new(CalcitTypeAnnotation::List(inner));
          }
          // No element type specified, use Dynamic
          return Arc::new(CalcitTypeAnnotation::List(Arc::new(Self::Dynamic)));
        }
        if is_map_tag(tag) {
          if tuple.extra.len() > 2 {
            eprintln!("[Warn] :map expects at most 2 arguments, got {}", tuple.extra.len());
          }
          let key_type = tuple
            .extra
            .first()
            .map(Self::parse_type_annotation_form)
            .unwrap_or_else(|| Arc::new(Self::Dynamic));
          let val_type = tuple
            .extra
            .get(1)
            .map(Self::parse_type_annotation_form)
            .unwrap_or_else(|| Arc::new(Self::Dynamic));
          return Arc::new(CalcitTypeAnnotation::Map(key_type, val_type));
        }
        if is_set_tag(tag) {
          if tuple.extra.len() > 1 {
            eprintln!("[Warn] :set expects at most 1 argument, got {}", tuple.extra.len());
          }
          if let Some(inner_form) = tuple.extra.first() {
            let inner = Self::parse_type_annotation_form(inner_form);
            return Arc::new(CalcitTypeAnnotation::Set(inner));
          }
          return Arc::new(CalcitTypeAnnotation::Set(Arc::new(Self::Dynamic)));
        }
        if is_ref_tag(tag) {
          if tuple.extra.len() > 1 {
            eprintln!("[Warn] :ref expects at most 1 argument, got {}", tuple.extra.len());
          }
          if let Some(inner_form) = tuple.extra.first() {
            let inner = Self::parse_type_annotation_form(inner_form);
            return Arc::new(CalcitTypeAnnotation::Ref(inner));
          }
          return Arc::new(CalcitTypeAnnotation::Ref(Arc::new(Self::Dynamic)));
        }
        if is_typeref_tag(tag) {
          let ns = tuple.extra.first().and_then(|x| get_str_from_calcit(x)).unwrap_or_default();
          let name = tuple.extra.get(1).and_then(|x| get_str_from_calcit(x)).unwrap_or_default();
          return Arc::new(CalcitTypeAnnotation::TypeRef { ns, name });
        }
      }
    }

    if let Calcit::List(xs) = form {
      if let Some(Calcit::Tag(tag)) = xs.first() {
        let tag_name = tag.ref_str().trim_start_matches(':');
        if is_optional_tag(tag) {
          if xs.len() != 2 {
            eprintln!("[Warn] :optional expects 1 argument, got {}", xs.len() as i64 - 1);
          }
          if let Some(inner_form) = xs.get(1) {
            let inner = Self::parse_type_annotation_form(inner_form);
            return Arc::new(CalcitTypeAnnotation::Optional(inner));
          }
        }
        if is_list_tag(tag) {
          if xs.len() > 2 {
            eprintln!("[Warn] :list expects at most 1 argument, got {}", xs.len() as i64 - 1);
          }
          if let Some(inner_form) = xs.get(1) {
            let inner = Self::parse_type_annotation_form(inner_form);
            return Arc::new(CalcitTypeAnnotation::List(inner));
          }
          return Arc::new(CalcitTypeAnnotation::List(Arc::new(Self::Dynamic)));
        }
        if is_map_tag(tag) {
          if xs.len() > 3 {
            eprintln!("[Warn] :map expects at most 2 arguments, got {}", xs.len() as i64 - 1);
          }
          let key_type = xs
            .get(1)
            .map(Self::parse_type_annotation_form)
            .unwrap_or_else(|| Arc::new(Self::Dynamic));
          let val_type = xs
            .get(2)
            .map(Self::parse_type_annotation_form)
            .unwrap_or_else(|| Arc::new(Self::Dynamic));
          return Arc::new(CalcitTypeAnnotation::Map(key_type, val_type));
        }
        if is_set_tag(tag) {
          if xs.len() > 2 {
            eprintln!("[Warn] :set expects at most 1 argument, got {}", xs.len() as i64 - 1);
          }
          if let Some(inner_form) = xs.get(1) {
            let inner = Self::parse_type_annotation_form(inner_form);
            return Arc::new(CalcitTypeAnnotation::Set(inner));
          }
          return Arc::new(CalcitTypeAnnotation::Set(Arc::new(Self::Dynamic)));
        }
        if is_ref_tag(tag) {
          if xs.len() > 2 {
            eprintln!("[Warn] :ref expects at most 1 argument, got {}", xs.len() as i64 - 1);
          }
          if let Some(inner_form) = xs.get(1) {
            let inner = Self::parse_type_annotation_form(inner_form);
            return Arc::new(CalcitTypeAnnotation::Ref(inner));
          }
          return Arc::new(CalcitTypeAnnotation::Ref(Arc::new(Self::Dynamic)));
        }
        if is_typeref_tag(tag) {
          let ns = xs.get(1).and_then(|x| get_str_from_calcit(x)).unwrap_or_default();
          let name = xs.get(2).and_then(|x| get_str_from_calcit(x)).unwrap_or_default();
          return Arc::new(CalcitTypeAnnotation::TypeRef { ns, name });
        }
        if tag_name == "fn" {
          let args_form = xs.get(1).unwrap_or(&Calcit::Nil);
          let arg_types = if let Calcit::List(args) = args_form {
            args.iter().map(Self::parse_type_annotation_form).collect()
          } else {
            vec![]
          };
          let return_type = xs
            .get(2)
            .map(Self::parse_type_annotation_form)
            .unwrap_or_else(|| Arc::new(Self::Dynamic));
          return Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
            arg_types,
            return_type,
          })));
        }
      }

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

        if let Some(Calcit::Tag(tag)) = xs.get(1) {
          let tag_name = tag.ref_str().trim_start_matches(':');
          if tag_name == "record" {
            if xs.len() < 3 {
              eprintln!("[Warn] :: :record expects struct name, got {}", xs.len() as i64 - 2);
            } else if let Some(record) = resolve_record_annotation(xs.get(2).unwrap(), xs.get(3)) {
              return Arc::new(CalcitTypeAnnotation::Record(Arc::new(record)));
            }
          }
          if tag_name == "tuple" {
            if xs.len() < 3 {
              eprintln!("[Warn] :: :tuple expects enum name, got {}", xs.len() as i64 - 2);
            } else if let Some(tuple) = resolve_tuple_annotation(xs.get(2).unwrap(), xs.get(3)) {
              return Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(tuple)));
            }
          }
          if tag_name == "list" {
            if let Some(inner_form) = xs.get(2) {
              let inner = Self::parse_type_annotation_form(inner_form);
              return Arc::new(CalcitTypeAnnotation::List(inner));
            }
            return Arc::new(CalcitTypeAnnotation::List(Arc::new(Self::Dynamic)));
          }
          if tag_name == "map" {
            let key_type = xs
              .get(2)
              .map(Self::parse_type_annotation_form)
              .unwrap_or_else(|| Arc::new(Self::Dynamic));
            let val_type = xs
              .get(3)
              .map(Self::parse_type_annotation_form)
              .unwrap_or_else(|| Arc::new(Self::Dynamic));
            return Arc::new(CalcitTypeAnnotation::Map(key_type, val_type));
          }
          if tag_name == "set" {
            if let Some(inner_form) = xs.get(2) {
              let inner = Self::parse_type_annotation_form(inner_form);
              return Arc::new(CalcitTypeAnnotation::Set(inner));
            }
            return Arc::new(CalcitTypeAnnotation::Set(Arc::new(Self::Dynamic)));
          }
          if tag_name == "ref" {
            if let Some(inner_form) = xs.get(2) {
              let inner = Self::parse_type_annotation_form(inner_form);
              return Arc::new(CalcitTypeAnnotation::Ref(inner));
            }
            return Arc::new(CalcitTypeAnnotation::Ref(Arc::new(Self::Dynamic)));
          }
          if tag_name == "typeref" {
            let ns = xs.get(2).and_then(|x| get_str_from_calcit(x)).unwrap_or_default();
            let name = xs.get(3).and_then(|x| get_str_from_calcit(x)).unwrap_or_default();
            return Arc::new(CalcitTypeAnnotation::TypeRef { ns, name });
          }
          if tag_name == "fn" {
            let args_form = xs.get(2).unwrap_or(&Calcit::Nil);
            let arg_types = if let Calcit::List(args) = args_form {
              args.iter().map(Self::parse_type_annotation_form).collect()
            } else {
              vec![]
            };
            let return_type = xs
              .get(3)
              .map(Self::parse_type_annotation_form)
              .unwrap_or_else(|| Arc::new(Self::Dynamic));
            return Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
              arg_types,
              return_type,
            })));
          }
        }
      }
    }

    // Handle symbol as type reference (e.g., MyStruct, ns/MyStruct)
    if let Calcit::Symbol { sym, .. } = form {
      let sym_str = sym.as_ref();
      // Check if it's a namespaced type reference (ns/name)
      if let Some((ns, name)) = sym_str.split_once('/') {
        return Arc::new(CalcitTypeAnnotation::TypeRef {
          ns: Arc::from(ns),
          name: Arc::from(name),
        });
      }
      // Simple type reference (name)
      return Arc::new(CalcitTypeAnnotation::TypeRef {
        ns: Arc::from(""),
        name: Arc::from(sym_str),
      });
    }

    Arc::new(CalcitTypeAnnotation::from_calcit(form))
  }

  fn tuple_tag_is_wildcard(tuple: &CalcitTuple) -> bool {
    match tuple.tag.as_ref() {
      Calcit::Tag(tag) => tag.ref_str().trim_start_matches(':') == "unknown",
      _ => false,
    }
  }

  fn tuple_matches(actual: &CalcitTuple, expected: &CalcitTuple) -> bool {
    if let Some(expected_enum) = &expected.sum_type {
      match &actual.sum_type {
        Some(actual_enum) if actual_enum.name() == expected_enum.name() => {}
        _ => return false,
      }
    }

    if let Some(expected_class) = &expected.class {
      match &actual.class {
        Some(actual_class) if actual_class.name() == expected_class.name() => {}
        _ => return false,
      }
    }

    if Self::tuple_tag_is_wildcard(expected) {
      return expected.extra.is_empty();
    }

    actual.tag == expected.tag && actual.extra == expected.extra
  }

  /// Render a concise representation used in warnings or logs
  pub fn to_brief_string(&self) -> String {
    if let Some(tag) = self.builtin_tag_name() {
      return format!(":{tag}");
    }

    match self {
      Self::Record(record) => format!("record {}", record.name()),
      Self::Tuple(_) => "tuple".to_string(),
      Self::Fn(signature) => signature.render_signature_brief(),
      Self::Variadic(inner) => format!("&{}", inner.to_brief_string()),
      Self::List(inner) => format!("list<{}>", inner.to_brief_string()),
      Self::Map(k, v) => format!("map<{},{}>", k.to_brief_string(), v.to_brief_string()),
      Self::Set(inner) => format!("set<{}>", inner.to_brief_string()),
      Self::Ref(inner) => format!("ref<{}>", inner.to_brief_string()),
      Self::Custom(inner) => format!("{inner}"),
      Self::Optional(inner) => format!("{}?", inner.to_brief_string()),
      Self::Dynamic => "any".to_string(),
      _ => "unknown".to_string(),
    }
  }

  pub fn matches_annotation(&self, expected: &CalcitTypeAnnotation) -> bool {
    match (self, expected) {
      (_, Self::Dynamic) | (Self::Dynamic, _) => true,
      (_, Self::Optional(expected_inner)) => match self {
        Self::Optional(actual_inner) => actual_inner.matches_annotation(expected_inner),
        _ => self.matches_annotation(expected_inner),
      },
      (Self::Optional(_), _) => false,
      (Self::Bool, Self::Bool)
      | (Self::Number, Self::Number)
      | (Self::String, Self::String)
      | (Self::Symbol, Self::Symbol)
      | (Self::Tag, Self::Tag)
      | (Self::DynFn, Self::DynFn)
      | (Self::Buffer, Self::Buffer)
      | (Self::CirruQuote, Self::CirruQuote) => true,
      (Self::List(a), Self::List(b)) => a.matches_annotation(b),
      (Self::Map(ak, av), Self::Map(bk, bv)) => ak.matches_annotation(bk) && av.matches_annotation(bv),
      (Self::Set(a), Self::Set(b)) => a.matches_annotation(b),
      (Self::Ref(a), Self::Ref(b)) => a.matches_annotation(b),
      (Self::Record(a), Self::Record(b)) => a.name() == b.name(),
      // Tuple type matching: DynTuple matches any Tuple, specific Tuple must match structure
      (Self::Tuple(_), Self::DynTuple) | (Self::DynTuple, Self::Tuple(_)) | (Self::DynTuple, Self::DynTuple) => true,
      (Self::Tuple(actual), Self::Tuple(expected)) => Self::tuple_matches(actual.as_ref(), expected.as_ref()),
      // Function type matching: DynFn matches any Fn, specific Fn must match signature
      (Self::Fn(_), Self::DynFn) | (Self::DynFn, Self::Fn(_)) => true,
      (Self::Fn(a), Self::Fn(b)) => a.matches_signature(b.as_ref()),
      (Self::Variadic(a), Self::Variadic(b)) => a.matches_annotation(b),
      (Self::Custom(a), Self::Custom(b)) => a.as_ref() == b.as_ref(),
      _ => false,
    }
  }

  pub fn from_calcit(value: &Calcit) -> Self {
    match value {
      Calcit::Nil => Self::Dynamic,
      Calcit::Bool(_) => Self::Bool,
      Calcit::Number(_) => Self::Number,
      Calcit::Str(_) => Self::String,
      Calcit::Tag(tag) => {
        let tag_name = tag.ref_str().trim_start_matches(':');
        if tag_name == "any" || tag_name == "dynamic" {
          Self::Dynamic
        } else if let Some(builtin) = Self::builtin_type_from_tag_name(tag_name) {
          builtin
        } else {
          Self::Tag
        }
      }
      Calcit::List(_) => Self::List(Arc::new(Self::Dynamic)),
      Calcit::Map(_) => Self::Map(Arc::new(Self::Dynamic), Arc::new(Self::Dynamic)),
      Calcit::Set(_) => Self::Set(Arc::new(Self::Dynamic)),
      Calcit::Record(record) => Self::Record(Arc::new(record.to_owned())),
      Calcit::Enum(enum_def) => Self::Record(Arc::new(enum_def.prototype().to_owned())),
      Calcit::Struct(struct_def) => {
        let values = vec![Calcit::Nil; struct_def.fields.len()];
        Self::Record(Arc::new(CalcitRecord {
          struct_ref: Arc::new(struct_def.to_owned()),
          values: Arc::new(values),
          class: None,
        }))
      }
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
      Calcit::Ref(_, _) => Self::Ref(Arc::new(Self::Dynamic)),
      Calcit::Symbol { .. } => Self::Symbol,
      Calcit::Buffer(_) => Self::Buffer,
      Calcit::CirruQuote(_) => Self::CirruQuote,
      other => Self::Custom(Arc::new(other.to_owned())),
    }
  }

  pub fn from_tag_name(name: &str) -> Self {
    let tag_name = name.trim_start_matches(':');
    if tag_name == "any" || tag_name == "dynamic" {
      Self::Dynamic
    } else {
      Self::builtin_type_from_tag_name(tag_name).unwrap_or(Self::Tag)
    }
  }

  pub fn from_function_parts(arg_types: Vec<Arc<CalcitTypeAnnotation>>, return_type: Arc<CalcitTypeAnnotation>) -> Self {
    Self::Fn(Arc::new(CalcitFnTypeAnnotation { arg_types, return_type }))
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
    if let Some(tag) = self.builtin_tag_name() {
      return Calcit::Tag(EdnTag::from(tag));
    }

    match self {
      Self::Record(record) => Calcit::Record((**record).clone()),
      Self::Tuple(tuple) => Calcit::Tuple((**tuple).clone()),
      Self::Fn(_) => Calcit::Tag(EdnTag::from("fn")),
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
      _ => Calcit::Nil,
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

  pub fn as_function(&self) -> Option<&CalcitFnTypeAnnotation> {
    match self {
      Self::Fn(signature) => Some(signature.as_ref()),
      Self::Optional(inner) => inner.as_function(),
      _ => None,
    }
  }

  pub fn describe(&self) -> String {
    match self {
      Self::List(inner) => {
        if matches!(inner.as_ref(), Self::Dynamic) {
          return "list".to_string();
        }
        return format!("list<{}>", inner.describe());
      }
      Self::Map(k, v) => {
        if matches!(k.as_ref(), Self::Dynamic) && matches!(v.as_ref(), Self::Dynamic) {
          return "map".to_string();
        }
        return format!("map<{}, {}>", k.describe(), v.describe());
      }
      Self::Set(inner) => {
        if matches!(inner.as_ref(), Self::Dynamic) {
          return "set".to_string();
        }
        return format!("set<{}>", inner.describe());
      }
      Self::Ref(inner) => {
        if matches!(inner.as_ref(), Self::Dynamic) {
          return "ref".to_string();
        }
        return format!("ref<{}>", inner.describe());
      }
      _ => {}
    }

    if let Some(tag) = self.builtin_tag_name() {
      return tag.to_string();
    }

    match self {
      Self::Record(record) => format!("record {}", record.name()),
      Self::Tuple(tuple) => format!("tuple {:?}", tuple.tag),
      Self::Fn(signature) => signature.describe(),
      Self::Variadic(inner) => format!("variadic {}", inner.describe()),
      Self::Custom(_) => "custom".to_string(),
      Self::Optional(inner) => format!("optional<{}>", inner.describe()),
      Self::Dynamic => "dynamic".to_string(),
      Self::TypeRef { ns, name } => format!("{}/{}", ns, name),
      _ => "unknown".to_string(),
    }
  }

  fn variant_order(&self) -> u8 {
    match self {
      Self::Bool => 1,
      Self::Number => 2,
      Self::String => 3,
      Self::Symbol => 4,
      Self::Tag => 5,
      Self::List(_) => 6,
      Self::Map(_, _) => 7,
      Self::DynFn => 8,
      Self::Ref(_) => 9,
      Self::Buffer => 10,
      Self::CirruQuote => 11,
      Self::Record(_) => 12,
      Self::Tuple(_) => 13,
      Self::DynTuple => 14,
      Self::Fn(_) => 15,
      Self::Set(_) => 16,
      Self::Variadic(_) => 17,
      Self::Custom(_) => 18,
      Self::Optional(_) => 19,
      Self::Dynamic => 20,
      Self::TypeRef { .. } => 21,
    }
  }
}

fn resolve_record_annotation(struct_form: &Calcit, class_form: Option<&Calcit>) -> Option<CalcitRecord> {
  let struct_def = resolve_struct_def(struct_form)?;
  let field_count = struct_def.fields.len();
  let class_record = class_form.and_then(resolve_record_def).map(Arc::new);
  Some(CalcitRecord {
    struct_ref: Arc::new(struct_def),
    values: Arc::new(vec![Calcit::Nil; field_count]),
    class: class_record,
  })
}

fn resolve_tuple_annotation(enum_form: &Calcit, class_form: Option<&Calcit>) -> Option<CalcitTuple> {
  let enum_def = resolve_enum_def(enum_form)?;
  let class_record = class_form.and_then(resolve_record_def).map(Arc::new);
  Some(CalcitTuple {
    tag: Arc::new(Calcit::Tag(EdnTag::from("unknown"))),
    extra: vec![],
    class: class_record,
    sum_type: Some(Arc::new(enum_def)),
  })
}

fn resolve_struct_def(form: &Calcit) -> Option<CalcitStruct> {
  match form {
    Calcit::Struct(struct_def) => Some(struct_def.to_owned()),
    Calcit::Record(record) => Some(record.struct_ref.as_ref().to_owned()),
    _ => resolve_calcit_value(form).and_then(|value| match value {
      Calcit::Struct(struct_def) => Some(struct_def),
      Calcit::Record(record) => Some(record.struct_ref.as_ref().to_owned()),
      _ => None,
    }),
  }
}

fn resolve_enum_def(form: &Calcit) -> Option<CalcitEnum> {
  match form {
    Calcit::Enum(enum_def) => Some(enum_def.to_owned()),
    Calcit::Record(record) => CalcitEnum::from_record(record.to_owned()).ok(),
    _ => resolve_calcit_value(form).and_then(|value| match value {
      Calcit::Enum(enum_def) => Some(enum_def),
      Calcit::Record(record) => CalcitEnum::from_record(record).ok(),
      _ => None,
    }),
  }
}

fn resolve_record_def(form: &Calcit) -> Option<CalcitRecord> {
  match form {
    Calcit::Record(record) => Some(record.to_owned()),
    _ => resolve_calcit_value(form).and_then(|value| match value {
      Calcit::Record(record) => Some(record),
      _ => None,
    }),
  }
}

fn resolve_calcit_value(form: &Calcit) -> Option<Calcit> {
  match form {
    Calcit::Import(import) => {
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
        .map(|value| value.to_owned());

      if pushed {
        IMPORT_RESOLUTION_STACK.with(|stack| {
          let mut stack = stack.borrow_mut();
          let _ = stack.pop();
        });
      }

      resolved
    }
    Calcit::Symbol { sym, info, .. } => program::lookup_evaled_def(info.at_ns.as_ref(), sym)
      .or_else(|| program::lookup_def_code(info.at_ns.as_ref(), sym))
      .map(|value| value.to_owned()),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::CalcitSymbolInfo;

  fn symbol(name: &str) -> Calcit {
    Calcit::Symbol {
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests"),
        at_def: Arc::from("collect_arg_type_hints"),
      }),
      location: None,
    }
  }

  #[test]
  fn collect_arg_type_hints_keeps_non_variadic() {
    let body_items = vec![Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::AssertType, Arc::from("tests")),
      symbol("a"),
      Calcit::Tag(EdnTag::from("number")),
    ])))];

    let params = vec![Arc::from("a")];
    let arg_types = CalcitTypeAnnotation::collect_arg_type_hints_from_body(&body_items, &params);

    assert!(matches!(arg_types[0].as_ref(), CalcitTypeAnnotation::Number));
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
      Self::Bool => "bool".hash(state),
      Self::Number => "number".hash(state),
      Self::String => "string".hash(state),
      Self::Symbol => "symbol".hash(state),
      Self::Tag => "tag".hash(state),
      Self::List(inner) => {
        "list".hash(state);
        inner.hash(state);
      }
      Self::Map(k, v) => {
        "map".hash(state);
        k.hash(state);
        v.hash(state);
      }
      Self::Record(record) => {
        "record".hash(state);
        let record = record.as_ref();
        record.struct_ref.name.hash(state);
        record.struct_ref.fields.hash(state);
        record.values.hash(state);
      }
      Self::Tuple(tuple) => {
        "tuple".hash(state);
        let tuple = tuple.as_ref();
        tuple.tag.hash(state);
        tuple.extra.hash(state);
      }
      Self::DynTuple => "dyntuple".hash(state),
      Self::DynFn => "dynfn".hash(state),
      Self::Fn(signature) => {
        "function".hash(state);
        signature.arg_types.hash(state);
        signature.return_type.hash(state);
      }
      Self::Set(inner) => {
        "set".hash(state);
        inner.hash(state);
      }
      Self::Ref(inner) => {
        "ref".hash(state);
        inner.hash(state);
      }
      Self::Buffer => "buffer".hash(state),
      Self::CirruQuote => "cirru-quote".hash(state),
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
      Self::Dynamic => "dynamic".hash(state),
      Self::TypeRef { ns, name } => {
        "typeref".hash(state);
        ns.hash(state);
        name.hash(state);
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
      (Self::Bool, Self::Bool)
      | (Self::Number, Self::Number)
      | (Self::String, Self::String)
      | (Self::Symbol, Self::Symbol)
      | (Self::Tag, Self::Tag)
      | (Self::DynFn, Self::DynFn)
      | (Self::Buffer, Self::Buffer)
      | (Self::CirruQuote, Self::CirruQuote) => Ordering::Equal,
      (Self::List(a), Self::List(b)) => a.cmp(b),
      (Self::Map(ak, av), Self::Map(bk, bv)) => ak.cmp(bk).then_with(|| av.cmp(bv)),
      (Self::Record(a), Self::Record(b)) => {
        let a = a.as_ref();
        let b = b.as_ref();
        a.struct_ref
          .name
          .cmp(&b.struct_ref.name)
          .then_with(|| a.struct_ref.fields.cmp(&b.struct_ref.fields))
          .then_with(|| a.values.cmp(&b.values))
      }
      (Self::Tuple(a), Self::Tuple(b)) => {
        let a = a.as_ref();
        let b = b.as_ref();
        a.tag.cmp(&b.tag).then_with(|| a.extra.cmp(&b.extra))
      }
      (Self::Fn(a), Self::Fn(b)) => a.arg_types.cmp(&b.arg_types).then_with(|| a.return_type.cmp(&b.return_type)),
      (Self::Set(a), Self::Set(b)) => a.cmp(b),
      (Self::Ref(a), Self::Ref(b)) => a.cmp(b),
      (Self::Variadic(a), Self::Variadic(b)) => a.cmp(b),
      (Self::Custom(a), Self::Custom(b)) => a.cmp(b),
      (Self::Optional(a), Self::Optional(b)) => a.cmp(b),
      (Self::Dynamic, Self::Dynamic) => Ordering::Equal,
      (Self::TypeRef { ns: ans, name: anm }, Self::TypeRef { ns: bns, name: bnm }) => ans.cmp(bns).then_with(|| anm.cmp(bnm)),
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
