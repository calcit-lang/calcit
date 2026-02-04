use std::{
  cell::RefCell,
  cmp::Ordering,
  collections::HashMap,
  fmt,
  hash::{Hash, Hasher},
  sync::Arc,
};

use std::thread_local;

use cirru_edn::EdnTag;

use super::{
  CORE_NS, Calcit, CalcitEnum, CalcitImport, CalcitList, CalcitProc, CalcitRecord, CalcitStruct, CalcitSymbolInfo, CalcitSyntax,
  CalcitTuple,
};
use crate::program;

thread_local! {
  static IMPORT_RESOLUTION_STACK: RefCell<Vec<(Arc<str>, Arc<str>)>> = const { RefCell::new(vec![]) };
}

pub(crate) type TypeBindings = HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>;

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
  /// Struct type definition used as a type annotation
  Struct(Arc<CalcitStruct>),
  /// Enum type definition used as a type annotation
  Enum(Arc<CalcitEnum>),
  /// Generic type variable, e.g. 'T
  TypeVar(Arc<str>),
  /// Struct type with applied generic arguments
  AppliedStruct {
    base: Arc<CalcitStruct>,
    args: Arc<Vec<Arc<CalcitTypeAnnotation>>>,
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

  fn parse_type_var_form(form: &Calcit) -> Option<Arc<str>> {
    let Calcit::List(list) = form else {
      return None;
    };

    let head = list.first()?;
    let is_quote_head = matches!(head, Calcit::Syntax(CalcitSyntax::Quote, _))
      || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "quote")
      || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "quote");

    if !is_quote_head {
      return None;
    }

    match list.get(1) {
      Some(Calcit::Symbol { sym, .. }) => Some(sym.to_owned()),
      _ => None,
    }
  }

  pub fn extract_return_type_from_hint_form(form: &Calcit) -> Option<Arc<CalcitTypeAnnotation>> {
    let list = match form {
      Calcit::List(xs) => xs,
      _ => return None,
    };

    let is_hint_fn = match list.first() {
      Some(Calcit::Syntax(CalcitSyntax::HintFn, _)) => true,
      Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "hint-fn" => true,
      _ => false,
    };

    if !is_hint_fn {
      return None;
    }

    let items = list.skip(1).ok()?;
    for item in items.iter() {
      let Calcit::List(inner) = item else {
        continue;
      };
      let head = inner.first();
      if matches!(head, Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "return-type") {
        if let Some(type_expr) = inner.get(1) {
          return Some(CalcitTypeAnnotation::parse_type_annotation_form(type_expr));
        }
      }
    }
    None
  }

  pub fn extract_generics_from_hint_form(form: &Calcit) -> Option<Vec<Arc<str>>> {
    let list = match form {
      Calcit::List(xs) => xs,
      _ => return None,
    };

    let is_hint_fn = match list.first() {
      Some(Calcit::Syntax(CalcitSyntax::HintFn, _)) => true,
      Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "hint-fn" => true,
      _ => false,
    };

    if !is_hint_fn {
      return None;
    }

    let items = list.skip(1).ok()?;
    for item in items.iter() {
      let Calcit::List(inner) = item else {
        continue;
      };
      let head = inner.first();
      if matches!(head, Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "type-vars") {
        let mut vars = vec![];
        for entry in inner.iter().skip(1) {
          vars.push(Self::parse_type_var_form(entry)?);
        }
        return Some(vars);
      }
    }
    None
  }

  fn parse_generics_list(form: &Calcit) -> Option<Vec<Arc<str>>> {
    let Calcit::List(items) = form else {
      return None;
    };

    let mut vars = Vec::with_capacity(items.len());
    for item in items.iter() {
      if let Some(name) = Self::parse_type_var_form(item) {
        vars.push(name);
        continue;
      }
      if let Calcit::Symbol { sym, .. } = item {
        vars.push(sym.to_owned());
        continue;
      }
      return None;
    }
    Some(vars)
  }

  /// Summarize definition code for `cr query def` output.
  ///
  /// Note: editor mode has no macro expansion, so only display what can be
  /// statically observed (e.g. `hint-fn`, `assert-type`). If no hints are found,
  /// return `None` to avoid noisy output.
  pub fn summarize_code(code: &Calcit) -> Option<String> {
    let mut list: &CalcitList = match code {
      Calcit::List(xs) => xs,
      _ => return None,
    };

    if list.is_empty() {
      return None;
    }

    // Snapshot code is often wrapped with (quote ...), unwrap if possible.
    let is_quote_head = match list.first() {
      Some(Calcit::Syntax(CalcitSyntax::Quote, _)) => true,
      Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "quote" => true,
      Some(Calcit::Import(CalcitImport { ns, def, .. })) if &**ns == CORE_NS && &**def == "quote" => true,
      _ => false,
    };

    if is_quote_head {
      if list.len() == 2 {
        if let Some(Calcit::List(inner)) = list.get(1) {
          list = inner;
        } else {
          return None;
        }
      } else {
        return None;
      }
    }

    let head = list.first()?;
    let is_defn =
      matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "defn") || matches!(head, Calcit::Syntax(CalcitSyntax::Defn, _));
    let is_defmacro = matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "defmacro")
      || matches!(head, Calcit::Syntax(CalcitSyntax::Defmacro, _));
    if is_defn || is_defmacro {
      let mut generics = vec![];
      let mut return_type = Arc::new(Self::Dynamic);
      let mut arg_names = vec![];
      let mut arg_types = HashMap::new();

      if let Some(Calcit::List(args)) = list.get(2) {
        for arg in args.iter() {
          if let Calcit::Symbol { sym, .. } = arg {
            arg_names.push(sym.to_owned());
          }
        }
      }

      // Scan body forms for available hints only; do not expand macros.
      for i in 3..list.len() {
        if let Some(form) = list.get(i) {
          if let Some(g) = Self::extract_generics_from_hint_form(form) {
            generics = g;
          }
          if let Some(ret) = Self::extract_return_type_from_hint_form(form) {
            return_type = ret;
          }
          if let Calcit::List(inner) = form {
            let is_assert = match inner.first() {
              Some(Calcit::Syntax(CalcitSyntax::AssertType, _)) => true,
              Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "assert-type" => true,
              _ => false,
            };
            if is_assert && inner.len() == 3 {
              if let (Some(Calcit::Symbol { sym, .. }), Some(type_form)) = (inner.get(1), inner.get(2)) {
                let t = Self::parse_type_annotation_form(type_form);
                arg_types.insert(sym.to_owned(), t);
              }
            }
          }
        }
      }

      let mut final_arg_types = vec![];
      for name in &arg_names {
        final_arg_types.push(arg_types.get(name).cloned().unwrap_or_else(|| Arc::new(Self::Dynamic)));
      }

      let has_hints = !generics.is_empty()
        || !matches!(return_type.as_ref(), Self::Dynamic)
        || final_arg_types.iter().any(|t| !matches!(t.as_ref(), Self::Dynamic));
      if !has_hints {
        return None;
      }

      let signature = CalcitFnTypeAnnotation {
        generics: Arc::new(generics),
        arg_types: final_arg_types,
        return_type,
      };
      return Some(signature.render_signature_brief());
    }
    None
  }

  fn parse_struct_annotation_from_tuple(xs: &CalcitList) -> Option<Arc<CalcitTypeAnnotation>> {
    let struct_form = xs.get(1)?;
    let resolved = resolve_calcit_value(struct_form)?;
    let Calcit::Struct(struct_def) = resolved else {
      return None;
    };

    let args = xs.iter().skip(2).map(Self::parse_type_annotation_form).collect::<Vec<_>>();
    Some(Arc::new(CalcitTypeAnnotation::AppliedStruct {
      base: Arc::new(struct_def),
      args: Arc::new(args),
    }))
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

    if matches!(form, Calcit::Nil) {
      return Arc::new(CalcitTypeAnnotation::Dynamic);
    }

    if let Some(type_var) = Self::parse_type_var_form(form) {
      return Arc::new(CalcitTypeAnnotation::TypeVar(type_var));
    }

    if let Calcit::Tuple(tuple) = form {
      if let Some(struct_def) = resolve_struct_def(tuple.tag.as_ref()) {
        let args = tuple.extra.iter().map(Self::parse_type_annotation_form).collect::<Vec<_>>();
        return Arc::new(CalcitTypeAnnotation::AppliedStruct {
          base: Arc::new(struct_def),
          args: Arc::new(args),
        });
      }
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
        if tag.ref_str().trim_start_matches(':') == "fn" {
          let mut generics: Vec<Arc<str>> = vec![];
          let (args_form, return_form) = if let Some(generic_vars) = tuple.extra.get(0).and_then(Self::parse_generics_list) {
            generics = generic_vars;
            (tuple.extra.get(1).unwrap_or(&Calcit::Nil), tuple.extra.get(2))
          } else {
            (tuple.extra.get(0).unwrap_or(&Calcit::Nil), tuple.extra.get(1))
          };
          let arg_types = if let Calcit::List(args) = args_form {
            args.iter().map(Self::parse_type_annotation_form).collect()
          } else {
            vec![]
          };
          let return_type = return_form
            .map(Self::parse_type_annotation_form)
            .unwrap_or_else(|| Arc::new(Self::Dynamic));
          return Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
            generics: Arc::new(generics),
            arg_types,
            return_type,
          })));
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
        if tag_name == "fn" {
          let mut generics: Vec<Arc<str>> = vec![];
          let (args_form, return_form) = if let Some(generic_vars) = xs.get(1).and_then(Self::parse_generics_list) {
            generics = generic_vars;
            (xs.get(2).unwrap_or(&Calcit::Nil), xs.get(3))
          } else {
            (xs.get(1).unwrap_or(&Calcit::Nil), xs.get(2))
          };

          let arg_types = if let Calcit::List(args) = args_form {
            args.iter().map(Self::parse_type_annotation_form).collect()
          } else {
            vec![]
          };
          let return_type = return_form
            .map(Self::parse_type_annotation_form)
            .unwrap_or_else(|| Arc::new(Self::Dynamic));
          return Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
            generics: Arc::new(generics),
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
            } else if let Some(struct_def) = resolve_struct_annotation(xs.get(2).unwrap(), xs.get(3)) {
              return Arc::new(CalcitTypeAnnotation::Struct(Arc::new(struct_def)));
            }
          }
          if tag_name == "tuple" {
            if xs.len() < 3 {
              eprintln!("[Warn] :: :tuple expects enum name, got {}", xs.len() as i64 - 2);
            } else if let Some(enum_def) = resolve_enum_annotation(xs.get(2).unwrap(), xs.get(3)) {
              return Arc::new(CalcitTypeAnnotation::Enum(Arc::new(enum_def)));
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
          if tag_name == "fn" {
            let mut generics: Vec<Arc<str>> = vec![];
            let (args_form, return_form) = if let Some(generic_vars) = xs.get(2).and_then(Self::parse_generics_list) {
              generics = generic_vars;
              (xs.get(3).unwrap_or(&Calcit::Nil), xs.get(4))
            } else {
              (xs.get(2).unwrap_or(&Calcit::Nil), xs.get(3))
            };
            let arg_types = if let Calcit::List(args) = args_form {
              args.iter().map(Self::parse_type_annotation_form).collect()
            } else {
              vec![]
            };
            let return_type = return_form
              .map(Self::parse_type_annotation_form)
              .unwrap_or_else(|| Arc::new(Self::Dynamic));
            return Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
              generics: Arc::new(generics),
              arg_types,
              return_type,
            })));
          }
        }

        if let Some(struct_annotation) = Self::parse_struct_annotation_from_tuple(xs) {
          return struct_annotation;
        }
      }
    }

    // Resolve symbols or imports to struct/enum definitions for type annotations.
    if let Some(resolved) = resolve_calcit_value(form) {
      match resolved {
        Calcit::Struct(struct_def) => return Arc::new(CalcitTypeAnnotation::Struct(Arc::new(struct_def))),
        Calcit::Enum(enum_def) => return Arc::new(CalcitTypeAnnotation::Enum(Arc::new(enum_def))),
        _ => {}
      }
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

    if let Some(expected_impl) = expected.impls.first() {
      match actual.impls.first() {
        Some(actual_impl) if actual_impl.name() == expected_impl.name() => {}
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
      Self::Struct(struct_def) => format!("struct {}", struct_def.name),
      Self::TypeVar(name) => format!("'{name}"),
      Self::AppliedStruct { base, args } => {
        if args.is_empty() {
          format!("struct {}", base.name)
        } else {
          let rendered = args.iter().map(|t| t.to_brief_string()).collect::<Vec<_>>().join(", ");
          format!("struct {}<{}>", base.name, rendered)
        }
      }
      Self::Enum(enum_def) => format!("enum {}", enum_def.name()),
      Self::Dynamic => "any".to_string(),
      _ => "unknown".to_string(),
    }
  }

  pub fn matches_annotation(&self, expected: &CalcitTypeAnnotation) -> bool {
    let mut bindings = TypeBindings::new();
    self.matches_with_bindings(expected, &mut bindings)
  }

  pub(crate) fn matches_with_bindings(&self, expected: &CalcitTypeAnnotation, bindings: &mut TypeBindings) -> bool {
    match (self, expected) {
      (_, Self::Dynamic) | (Self::Dynamic, _) => true,
      (_, Self::Optional(expected_inner)) => match self {
        Self::Optional(actual_inner) => actual_inner.matches_with_bindings(expected_inner, bindings),
        _ => self.matches_with_bindings(expected_inner, bindings),
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
      (actual, Self::TypeVar(var)) => match bindings.get(var) {
        Some(bound) => {
          let bound = bound.clone();
          actual.matches_with_bindings(bound.as_ref(), bindings)
        }
        None => {
          bindings.insert(var.to_owned(), Arc::new(actual.to_owned()));
          true
        }
      },
      (Self::TypeVar(var), expected_type) => match bindings.get(var) {
        Some(bound) => {
          let bound = bound.clone();
          bound.as_ref().matches_with_bindings(expected_type, bindings)
        }
        None => {
          bindings.insert(var.to_owned(), Arc::new(expected_type.to_owned()));
          true
        }
      },
      (Self::List(a), Self::List(b)) => a.matches_with_bindings(b, bindings),
      (Self::Map(ak, av), Self::Map(bk, bv)) => ak.matches_with_bindings(bk, bindings) && av.matches_with_bindings(bv, bindings),
      (Self::Set(a), Self::Set(b)) => a.matches_with_bindings(b, bindings),
      (Self::Ref(a), Self::Ref(b)) => a.matches_with_bindings(b, bindings),
      (Self::Record(a), Self::Record(b)) => a.name() == b.name(),
      (Self::Struct(a), Self::Struct(b)) => a.name == b.name,
      (Self::Enum(a), Self::Enum(b)) => a.name() == b.name(),
      (Self::Record(a), Self::Struct(b)) => a.struct_ref.name == b.name,
      (Self::Record(a), Self::AppliedStruct { base, args }) => {
        if a.struct_ref.name != base.name {
          return false;
        }
        for (idx, arg) in args.iter().enumerate() {
          let expected = base.generics.get(idx);
          if let Some(var_name) = expected {
            let var = Arc::new(CalcitTypeAnnotation::TypeVar(var_name.to_owned()));
            if !arg.matches_with_bindings(var.as_ref(), bindings) {
              return false;
            }
          }
        }
        true
      }
      (Self::Tuple(a), Self::Enum(b)) => match &a.sum_type {
        Some(sum_type) => sum_type.name() == b.name(),
        None => false,
      },
      (Self::AppliedStruct { base, args }, Self::Struct(other)) | (Self::Struct(other), Self::AppliedStruct { base, args }) => {
        if base.name != other.name {
          return false;
        }
        for (idx, arg) in args.iter().enumerate() {
          let expected = base.generics.get(idx);
          if let Some(var_name) = expected {
            let var = Arc::new(CalcitTypeAnnotation::TypeVar(var_name.to_owned()));
            if !arg.matches_with_bindings(var.as_ref(), bindings) {
              return false;
            }
          }
        }
        true
      }
      (Self::AppliedStruct { base: a, args: a_args }, Self::AppliedStruct { base: b, args: b_args }) => {
        if a.name != b.name {
          return false;
        }
        if a_args.len() != b_args.len() {
          return false;
        }
        for (lhs, rhs) in a_args.iter().zip(b_args.iter()) {
          if !lhs.matches_with_bindings(rhs, bindings) {
            return false;
          }
        }
        true
      }
      // Tuple type matching: DynTuple matches any Tuple, specific Tuple must match structure
      (Self::Tuple(_), Self::DynTuple) | (Self::DynTuple, Self::Tuple(_)) | (Self::DynTuple, Self::DynTuple) => true,
      (Self::Tuple(actual), Self::Tuple(expected)) => Self::tuple_matches(actual.as_ref(), expected.as_ref()),
      // Function type matching: DynFn matches any Fn, specific Fn must match signature
      (Self::Fn(_), Self::DynFn) | (Self::DynFn, Self::Fn(_)) => true,
      (Self::Fn(a), Self::Fn(b)) => a.matches_signature(b.as_ref()),
      (Self::Variadic(a), Self::Variadic(b)) => a.matches_with_bindings(b, bindings),
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
      Calcit::Enum(enum_def) => Self::Enum(Arc::new(enum_def.to_owned())),
      Calcit::Struct(struct_def) => Self::Struct(Arc::new(struct_def.to_owned())),
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
    Self::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      arg_types,
      return_type,
    }))
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

  fn make_symbol(name: &str) -> Calcit {
    Calcit::Symbol {
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from(CORE_NS),
        at_def: Arc::from("type-annotation"),
      }),
      location: None,
    }
  }

  fn quote_symbol(name: &Arc<str>) -> Calcit {
    Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, Arc::from(CORE_NS)),
      Calcit::Symbol {
        sym: name.to_owned(),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from(CORE_NS),
          at_def: Arc::from("type-annotation"),
        }),
        location: None,
      },
    ])))
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
        impls: vec![],
        sum_type: None,
      }),
      Self::Custom(value) => value.as_ref().to_owned(),
      Self::Optional(inner) => Calcit::Tuple(CalcitTuple {
        tag: Arc::new(Calcit::Tag(EdnTag::from("optional"))),
        extra: vec![inner.to_calcit()],
        impls: vec![],
        sum_type: None,
      }),
      Self::Struct(struct_def) => Calcit::Struct((**struct_def).clone()),
      Self::TypeVar(name) => Self::quote_symbol(name),
      Self::AppliedStruct { base, args } => {
        let mut items = Vec::with_capacity(args.len() + 2);
        items.push(Self::make_symbol("::"));
        let base_name = base.name.ref_str().trim_start_matches(':');
        items.push(Self::make_symbol(base_name));
        for arg in args.iter() {
          items.push(arg.to_calcit());
        }
        Calcit::List(Arc::new(CalcitList::from(items.as_slice())))
      }
      Self::Enum(enum_def) => Calcit::Enum((**enum_def).clone()),
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
      Self::Struct(struct_def) => format!("struct {}", struct_def.name),
      Self::TypeVar(name) => format!("'{name}"),
      Self::AppliedStruct { base, args } => {
        if args.is_empty() {
          format!("struct {}", base.name)
        } else {
          let rendered = args.iter().map(|t| t.describe()).collect::<Vec<_>>().join(", ");
          format!("struct {}<{}>", base.name, rendered)
        }
      }
      Self::Enum(enum_def) => format!("enum {}", enum_def.name()),
      Self::Dynamic => "dynamic".to_string(),
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
      Self::TypeVar(_) => 21,
      Self::Struct(_) => 22,
      Self::AppliedStruct { .. } => 23,
      Self::Enum(_) => 24,
    }
  }
}

fn resolve_struct_annotation(struct_form: &Calcit, class_form: Option<&Calcit>) -> Option<CalcitStruct> {
  let mut struct_def = resolve_struct_def(struct_form)?;
  if let Some(class_record) = class_form.and_then(resolve_record_def) {
    struct_def.impls = vec![Arc::new(class_record)];
  }
  Some(struct_def)
}

fn resolve_enum_annotation(enum_form: &Calcit, class_form: Option<&Calcit>) -> Option<CalcitEnum> {
  let mut enum_def = resolve_enum_def(enum_form)?;
  if let Some(class_record) = class_form.and_then(resolve_record_def) {
    enum_def.set_impls(vec![Arc::new(class_record)]);
  }
  Some(enum_def)
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

fn resolve_type_def_from_code(code: &Calcit) -> Option<Calcit> {
  let Calcit::List(items) = code else {
    return None;
  };
  if let Some(head) = items.first() {
    if matches!(head, Calcit::Syntax(CalcitSyntax::Quote, _))
      || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "quote")
      || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "quote")
    {
      if let Some(inner) = items.get(1) {
        return resolve_type_def_from_code(inner);
      }
    }
  }
  let head = items.first()?;
  if is_defstruct_head(head) || is_struct_new_head(head) {
    return parse_defstruct_code(items.as_ref()).map(Calcit::Struct);
  }
  if is_defenum_head(head) || is_enum_new_head(head) {
    return parse_defenum_code(items.as_ref()).map(Calcit::Enum);
  }
  None
}

fn is_defstruct_head(head: &Calcit) -> bool {
  matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "defstruct")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "defstruct")
}

fn is_defenum_head(head: &Calcit) -> bool {
  matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "defenum")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "defenum")
}

fn is_struct_new_head(head: &Calcit) -> bool {
  matches!(head, Calcit::Proc(CalcitProc::NativeStructNew))
    || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "&struct::new")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "&struct::new")
}

fn is_enum_new_head(head: &Calcit) -> bool {
  matches!(head, Calcit::Proc(CalcitProc::NativeEnumNew))
    || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "&enum::new")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "&enum::new")
}

fn parse_type_name(form: &Calcit) -> Option<EdnTag> {
  match form {
    Calcit::Symbol { sym, .. } | Calcit::Str(sym) => Some(EdnTag::from(sym.as_ref())),
    Calcit::Tag(tag) => Some(tag.to_owned()),
    _ => None,
  }
}

fn is_list_literal_head(head: &Calcit) -> bool {
  matches!(head, Calcit::Proc(CalcitProc::List))
    || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "[]")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "[]")
}

fn parse_defstruct_code(items: &CalcitList) -> Option<CalcitStruct> {
  let name_form = items.get(1)?;
  let name = parse_type_name(name_form)?;
  let mut generics: Vec<Arc<str>> = vec![];
  let mut start_idx = 2;

  if let Some(generics_form) = items.get(2) {
    if let Some(vars) = CalcitTypeAnnotation::parse_generics_list(generics_form) {
      generics = vars;
      start_idx = 3;
    }
  }
  let mut fields: Vec<(EdnTag, Arc<CalcitTypeAnnotation>)> = Vec::new();

  for item in items.iter().skip(start_idx) {
    let Calcit::List(pair) = item else {
      return None;
    };
    let (field_name_form, field_type_form) = match pair.len() {
      2 => (pair.get(0)?, pair.get(1)?),
      3 if pair.first().is_some_and(is_list_literal_head) => (pair.get(1)?, pair.get(2)?),
      _ => return None,
    };
    let field_name = parse_type_name(field_name_form)?;
    let field_type = CalcitTypeAnnotation::parse_type_annotation_form(field_type_form);
    fields.push((field_name, field_type));
  }

  fields.sort_by(|a, b| a.0.ref_str().cmp(b.0.ref_str()));
  for idx in 1..fields.len() {
    if fields[idx - 1].0 == fields[idx].0 {
      return None;
    }
  }

  generics.sort();
  generics.dedup();

  let field_names: Vec<EdnTag> = fields.iter().map(|(name, _)| name.to_owned()).collect();
  let field_types: Vec<Arc<CalcitTypeAnnotation>> = fields.iter().map(|(_, t)| t.to_owned()).collect();

  Some(CalcitStruct {
    name,
    fields: Arc::new(field_names),
    field_types: Arc::new(field_types),
    generics: Arc::new(generics),
    impls: vec![],
  })
}

fn parse_defenum_code(items: &CalcitList) -> Option<CalcitEnum> {
  let name_form = items.get(1)?;
  let name = parse_type_name(name_form)?;

  let mut variants: Vec<(EdnTag, Calcit)> = Vec::new();
  for item in items.iter().skip(2) {
    let Calcit::List(variant) = item else {
      return None;
    };
    let tag_form = variant.first()?;
    let tag = parse_type_name(tag_form)?;
    let payloads: Vec<Calcit> = variant.iter().skip(1).map(|v| v.to_owned()).collect();
    let payload_value = if payloads.is_empty() {
      Calcit::Nil
    } else {
      Calcit::List(Arc::new(CalcitList::Vector(payloads)))
    };
    variants.push((tag, payload_value));
  }

  variants.sort_by(|a, b| a.0.ref_str().cmp(b.0.ref_str()));
  for idx in 1..variants.len() {
    if variants[idx - 1].0 == variants[idx].0 {
      return None;
    }
  }

  let fields: Vec<EdnTag> = variants.iter().map(|(tag, _)| tag.to_owned()).collect();
  let values: Vec<Calcit> = variants.iter().map(|(_, value)| value.to_owned()).collect();
  let struct_ref = CalcitStruct::from_fields(name, fields);
  let record = CalcitRecord {
    struct_ref: Arc::new(struct_ref),
    values: Arc::new(values),
    impls: vec![],
  };
  CalcitEnum::from_record(record).ok()
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
        .map(|value| resolve_type_def_from_code(&value).unwrap_or(value))
        .or_else(|| {
          program::lookup_def_code(import.ns.as_ref(), import.def.as_ref())
            .map(|value| resolve_type_def_from_code(&value).unwrap_or(value))
        });

      if pushed {
        IMPORT_RESOLUTION_STACK.with(|stack| {
          let mut stack = stack.borrow_mut();
          let _ = stack.pop();
        });
      }

      resolved
    }
    Calcit::Symbol { sym, info, .. } => program::lookup_evaled_def(info.at_ns.as_ref(), sym)
      .map(|value| resolve_type_def_from_code(&value).unwrap_or(value))
      .or_else(|| program::lookup_def_code(info.at_ns.as_ref(), sym).map(|value| resolve_type_def_from_code(&value).unwrap_or(value))),
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
      Self::Struct(struct_def) => {
        "struct".hash(state);
        struct_def.name.hash(state);
        struct_def.fields.hash(state);
        struct_def.field_types.hash(state);
      }
      Self::TypeVar(name) => {
        "typevar".hash(state);
        name.hash(state);
      }
      Self::AppliedStruct { base, args } => {
        "applied-struct".hash(state);
        base.name.hash(state);
        base.fields.hash(state);
        base.field_types.hash(state);
        base.generics.hash(state);
        args.hash(state);
      }
      Self::Enum(enum_def) => {
        "enum".hash(state);
        enum_def.name().hash(state);
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
      (Self::Struct(a), Self::Struct(b)) => a.name.cmp(&b.name).then_with(|| a.fields.cmp(&b.fields)),
      (Self::Enum(a), Self::Enum(b)) => a.name().cmp(b.name()),
      _ => Ordering::Equal, // other variants already separated by kind order
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalcitFnTypeAnnotation {
  pub generics: Arc<Vec<Arc<str>>>,
  pub arg_types: Vec<Arc<CalcitTypeAnnotation>>,
  pub return_type: Arc<CalcitTypeAnnotation>,
}

impl CalcitFnTypeAnnotation {
  pub fn describe(&self) -> String {
    let generics = if self.generics.is_empty() {
      "".to_string()
    } else {
      let rendered = self.generics.iter().map(|name| format!("'{name}")).collect::<Vec<_>>().join(", ");
      format!("<{rendered}>")
    };
    let args = if self.arg_types.is_empty() {
      "()".to_string()
    } else {
      let rendered = self.arg_types.iter().map(|t| t.describe()).collect::<Vec<_>>().join(", ");
      format!("({rendered})")
    };
    format!("fn{generics}{args} -> {}", self.return_type.describe())
  }

  pub fn render_signature_brief(&self) -> String {
    let generics = if self.generics.is_empty() {
      "".to_string()
    } else {
      let rendered = self.generics.iter().map(|name| format!("'{name}")).collect::<Vec<_>>().join(", ");
      format!("<{rendered}>")
    };
    let args_repr = if self.arg_types.is_empty() {
      "()".to_string()
    } else {
      let parts = self.arg_types.iter().map(|t| t.to_brief_string()).collect::<Vec<_>>().join(", ");
      format!("({parts})")
    };

    format!("fn{generics}{args_repr} -> {}", self.return_type.to_brief_string())
  }

  pub fn matches_signature(&self, other: &CalcitFnTypeAnnotation) -> bool {
    if self.arg_types.len() != other.arg_types.len() {
      return false;
    }

    if self.generics.len() != other.generics.len() {
      return false;
    }

    let mut bindings = TypeBindings::new();

    for (lhs, rhs) in self.arg_types.iter().zip(other.arg_types.iter()) {
      if !lhs.matches_with_bindings(rhs, &mut bindings) {
        return false;
      }
    }

    self.return_type.matches_with_bindings(other.return_type.as_ref(), &mut bindings)
  }
}
