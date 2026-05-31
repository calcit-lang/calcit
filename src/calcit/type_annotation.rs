use std::{
  cell::RefCell,
  cmp::Ordering,
  collections::HashMap,
  fmt,
  hash::{Hash, Hasher},
  sync::Arc,
};

use std::thread_local;

use cirru_edn::{Edn, EdnListView, EdnMapView, EdnTag};

use super::{
  CORE_NS, Calcit, CalcitEnum, CalcitImpl, CalcitImport, CalcitList, CalcitProc, CalcitRecord, CalcitStruct, CalcitSymbolInfo,
  CalcitSyntax, CalcitTrait, CalcitTuple,
};
use std::sync::{LazyLock, OnceLock};

// ---------------------------------------------------------------------------
// Decoupled program lookups – registered at runtime by `program::init_type_annotation_lookups()`
// to avoid a circular dependency: type_annotation → program → snapshot → calcit.
// ---------------------------------------------------------------------------
type LookupFn = fn(&str, &str) -> Option<Calcit>;
type SchemaLookupFn = fn(&str, &str) -> Arc<CalcitTypeAnnotation>;
static LOOKUP_RUNTIME_READY_DEF: OnceLock<LookupFn> = OnceLock::new();
static LOOKUP_DEF_CODE: OnceLock<LookupFn> = OnceLock::new();
static LOOKUP_DEF_SCHEMA: OnceLock<SchemaLookupFn> = OnceLock::new();
thread_local! {
  static TYPE_ANNOTATION_WARNING_CONTEXT: RefCell<Vec<Arc<str>>> = const { RefCell::new(vec![]) };
  /// Global type-slot registry: maps slot names to their bound type annotations.
  /// A slot is declared via `deftype-slot` (value = None) and bound via `bind-type` (value = Some).
  static TYPE_SLOTS: RefCell<HashMap<Arc<str>, Option<Arc<CalcitTypeAnnotation>>>> = RefCell::new(HashMap::new());
}

/// Register program-level lookup functions.  Must be called once at startup
/// (e.g. from `program::extract_program_data`) before any type-annotation
/// resolution that needs import-chain traversal.
pub fn register_program_lookups(runtime_ready_lookup: LookupFn, code_lookup: LookupFn, schema_lookup: SchemaLookupFn) {
  let _ = LOOKUP_RUNTIME_READY_DEF.set(runtime_ready_lookup);
  let _ = LOOKUP_DEF_CODE.set(code_lookup);
  let _ = LOOKUP_DEF_SCHEMA.set(schema_lookup);
}

pub fn with_type_annotation_warning_context<T>(label: impl Into<Arc<str>>, f: impl FnOnce() -> T) -> T {
  TYPE_ANNOTATION_WARNING_CONTEXT.with(|stack| stack.borrow_mut().push(label.into()));
  let result = f();
  TYPE_ANNOTATION_WARNING_CONTEXT.with(|stack| {
    stack.borrow_mut().pop();
  });
  result
}

fn current_type_annotation_warning_context() -> Option<Arc<str>> {
  TYPE_ANNOTATION_WARNING_CONTEXT.with(|stack| stack.borrow().last().cloned())
}

// ---------------------------------------------------------------------------
// Type-slot public API
// ---------------------------------------------------------------------------

/// Declare a type slot. Returns Err if the slot name is already declared.
pub fn register_type_slot(name: Arc<str>) -> Result<(), String> {
  TYPE_SLOTS.with(|slots| {
    let mut map = slots.borrow_mut();
    if map.contains_key(&name) {
      return Err(format!("type slot already declared: {name}"));
    }
    map.insert(name, None);
    Ok(())
  })
}

/// Bind a concrete type to a declared slot. Auto-registers the slot if not yet declared.
/// Returns Err if the slot is already bound (double-bind).
pub fn bind_type_slot(name: &str, ty: Arc<CalcitTypeAnnotation>) -> Result<(), String> {
  TYPE_SLOTS.with(|slots| {
    let mut map = slots.borrow_mut();
    match map.get_mut(name) {
      Some(slot) if slot.is_some() => Err(format!(
        "type slot '{name}' already bound — each slot can only be bound once per program"
      )),
      Some(slot) => {
        *slot = Some(ty);
        Ok(())
      }
      None => {
        map.insert(Arc::from(name), Some(ty));
        Ok(())
      }
    }
  })
}

/// Look up the type bound to a slot. Returns `None` if the slot is unknown or not yet bound.
pub fn resolve_type_slot(name: &str) -> Option<Arc<CalcitTypeAnnotation>> {
  TYPE_SLOTS.with(|slots| slots.borrow().get(name).and_then(|v| v.clone()))
}

/// Clear all type slots. Called at program startup/shutdown to avoid stale state across runs.
#[allow(dead_code)]
pub fn clear_type_slots() {
  TYPE_SLOTS.with(|slots| slots.borrow_mut().clear());
}

fn truncate_type_form_preview(raw: &str) -> String {
  const LIMIT: usize = 160;
  if raw.chars().count() > LIMIT {
    let truncated = raw.chars().take(LIMIT).collect::<String>();
    format!("{truncated}…")
  } else {
    raw.to_owned()
  }
}

fn emit_legacy_fn_type_syntax_warning(schema_hint: &str, form: &Calcit) {
  let preview = truncate_type_form_preview(&form.turn_string());
  if let Some(label) = current_type_annotation_warning_context() {
    eprintln!(
      "[Warn] legacy fn type syntax is no longer supported at {label} for `{preview}`, use `{schema_hint}` schema map form instead"
    );
  } else {
    eprintln!("[Warn] legacy fn type syntax is no longer supported for `{preview}`, use `{schema_hint}` schema map form instead");
  }
}

fn lookup_runtime_ready_registered(ns: &str, def: &str) -> Option<Calcit> {
  LOOKUP_RUNTIME_READY_DEF.get().and_then(|f| f(ns, def))
}

fn lookup_def_code_registered(ns: &str, def: &str) -> Option<Calcit> {
  LOOKUP_DEF_CODE.get().and_then(|f| f(ns, def))
}

/// Look up a definition's schema type annotation by namespace and definition name.
/// Returns `None` if the lookup function is not registered, or the schema is `Dynamic`.
fn lookup_schema_registered(ns: &str, def: &str) -> Option<Arc<CalcitTypeAnnotation>> {
  let schema = LOOKUP_DEF_SCHEMA.get().map(|f| f(ns, def))?;
  if matches!(schema.as_ref(), CalcitTypeAnnotation::Dynamic) {
    None
  } else {
    Some(schema)
  }
}

/// Try to resolve a TypeRef name (formatted as "ns/def") as a schema-based type alias.
/// This allows definitions with non-Dynamic schemas to serve as named type aliases.
fn resolve_type_ref_as_schema(name: &str) -> Option<Arc<CalcitTypeAnnotation>> {
  let (ns, def) = name.split_once('/')?;
  lookup_schema_registered(ns, def)
}

thread_local! {
  static IMPORT_RESOLUTION_STACK: RefCell<Vec<(Arc<str>, Arc<str>)>> = const { RefCell::new(vec![]) };
}

pub static DYNAMIC_TYPE: LazyLock<Arc<CalcitTypeAnnotation>> = LazyLock::new(|| Arc::new(CalcitTypeAnnotation::Dynamic));

pub(crate) type TypeBindings = HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>;

#[derive(Default)]
struct FnSchemaFields<'a> {
  has_any: bool,
  generics: Option<&'a Calcit>,
  where_clause: Option<&'a Calcit>,
  args: Option<&'a Calcit>,
  returns: Option<&'a Calcit>,
  rest: Option<&'a Calcit>,
  kind: Option<&'a Calcit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CalcitGenericBound {
  pub name: Arc<str>,
  pub traits: Arc<Vec<Arc<CalcitTrait>>>,
}

impl PartialOrd for CalcitGenericBound {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for CalcitGenericBound {
  fn cmp(&self, other: &Self) -> Ordering {
    self
      .name
      .cmp(&other.name)
      .then_with(|| self.to_brief_string().cmp(&other.to_brief_string()))
  }
}

impl CalcitGenericBound {
  pub fn as_type_annotation(&self) -> Arc<CalcitTypeAnnotation> {
    if self.traits.len() == 1 {
      Arc::new(CalcitTypeAnnotation::Trait(self.traits[0].clone()))
    } else {
      Arc::new(CalcitTypeAnnotation::TraitSet(self.traits.clone()))
    }
  }

  pub fn to_brief_string(&self) -> String {
    let rendered = self.traits.iter().map(|t| t.name.to_string()).collect::<Vec<_>>().join(" + ");
    format!("'{}: {rendered}", self.name)
  }
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
  /// A record value's type, identified by its struct definition
  Record(Arc<CalcitStruct>),
  /// A tuple value's type, optionally identified by its sum type enum
  Tuple(Arc<CalcitEnum>),
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
  /// Fallback for shapes that are not yet modeled explicitly as a record type
  Custom(Arc<Calcit>),
  /// No checking at static analaysis time
  Dynamic,
  /// Represents an type that can be nil or the given type
  Optional(Arc<CalcitTypeAnnotation>),
  /// Struct type definition, optionally with applied generic arguments.
  /// `args` is empty when used as a bare type annotation (no generics applied).
  Struct(Arc<CalcitStruct>, Arc<Vec<Arc<CalcitTypeAnnotation>>>),
  /// Enum type definition, optionally with applied generic arguments.
  Enum(Arc<CalcitEnum>, Arc<Vec<Arc<CalcitTypeAnnotation>>>),
  /// Generic type variable, e.g. 'T
  TypeVar(Arc<str>),
  /// Named type reference kept as source-level syntax, e.g. `'Result` or `(:: 'Result 'T 'E)`.
  ///
  /// This is used when a schema references another named definition but the annotation should
  /// remain a symbolic reference instead of collapsing into a generic variable or eagerly
  /// resolving into a concrete struct/enum definition.
  TypeRef(Arc<str>, Arc<Vec<Arc<CalcitTypeAnnotation>>>),
  /// Trait type annotation for trait objects
  Trait(Arc<CalcitTrait>),
  /// Multiple trait constraints recorded in order
  TraitSet(Arc<Vec<Arc<CalcitTrait>>>),
  /// Unit/nil type — for side-effectful functions that explicitly return nil
  Unit,
  /// A type slot reference declared via `deftype-slot` and bound via `bind-type`.
  /// At type-checking time, this is resolved by looking up the global TYPE_SLOTS registry.
  TypeSlot(Arc<str>),
}

impl CalcitTypeAnnotation {
  fn bind_declared_generics_from_applied_args(
    declared_generics: &[Arc<str>],
    applied_args: &[Arc<CalcitTypeAnnotation>],
    bindings: &mut TypeBindings,
  ) -> bool {
    for (idx, arg) in applied_args.iter().enumerate() {
      let Some(var_name) = declared_generics.get(idx) else {
        return false;
      };
      let var = Arc::new(CalcitTypeAnnotation::TypeVar(var_name.to_owned()));
      if !arg.matches_with_bindings(var.as_ref(), bindings) {
        return false;
      }
    }
    true
  }

  pub(crate) fn validate_applied_type_args(&self) -> Result<(), String> {
    match self {
      Self::List(inner) | Self::Set(inner) | Self::Ref(inner) | Self::Variadic(inner) | Self::Optional(inner) => {
        inner.validate_applied_type_args()
      }
      Self::Map(key, value) => {
        key.validate_applied_type_args()?;
        value.validate_applied_type_args()
      }
      Self::Fn(signature) => signature.validate_applied_type_args(),
      Self::Struct(base, args) => {
        for arg in args.iter() {
          arg.validate_applied_type_args()?;
        }

        let expected = base.generics.len();
        let actual = args.len();
        if expected == 0 {
          if actual > 0 {
            return Err(format!(
              "struct `{}` is not generic but received {actual} type argument(s)",
              base.name
            ));
          }
        } else if actual != expected {
          return Err(format!(
            "struct `{}` expects {expected} type argument(s), but received {actual}",
            base.name
          ));
        }

        Ok(())
      }
      Self::Enum(enum_def, args) => {
        for arg in args.iter() {
          arg.validate_applied_type_args()?;
        }

        let expected = enum_def.generics().len();
        let actual = args.len();
        if expected == 0 {
          if actual > 0 {
            return Err(format!(
              "enum `{}` is not generic but received {} type argument(s)",
              enum_def.name(),
              actual
            ));
          }
        } else if actual != expected {
          return Err(format!(
            "enum `{}` expects {expected} type argument(s), but received {actual}",
            enum_def.name(),
          ));
        }

        Ok(())
      }
      Self::TypeRef(_, args) => {
        for arg in args.iter() {
          arg.validate_applied_type_args()?;
        }
        Ok(())
      }
      Self::Record(_) | Self::Tuple(_) | Self::Trait(_) | Self::TraitSet(_) | Self::Custom(_) => Ok(()),
      Self::Bool
      | Self::Number
      | Self::String
      | Self::Symbol
      | Self::Tag
      | Self::DynTuple
      | Self::DynFn
      | Self::Buffer
      | Self::CirruQuote
      | Self::Dynamic
      | Self::TypeVar(_)
      | Self::Unit
      | Self::TypeSlot(_) => Ok(()),
    }
  }

  fn custom_keyword_matches(custom: &Calcit, keyword: &str) -> bool {
    match custom {
      Calcit::Tag(tag) => tag.ref_str().trim_start_matches(':') == keyword,
      _ => false,
    }
  }

  fn builtin_type_from_tag_name(name: &str) -> Option<Self> {
    match name {
      "bool" => Some(Self::Bool),
      "number" => Some(Self::Number),
      "string" => Some(Self::String),
      "symbol" => Some(Self::Symbol),
      "tag" => Some(Self::Tag),
      "list" => Some(Self::List(DYNAMIC_TYPE.clone())),
      "map" => Some(Self::Map(DYNAMIC_TYPE.clone(), DYNAMIC_TYPE.clone())),
      "set" => Some(Self::Set(DYNAMIC_TYPE.clone())),
      "tuple" => Some(Self::DynTuple),
      "fn" => Some(Self::DynFn),
      "ref" => Some(Self::Ref(DYNAMIC_TYPE.clone())),
      "buffer" => Some(Self::Buffer),
      "cirru-quote" => Some(Self::CirruQuote),
      "unit" | "nil" => Some(Self::Unit),
      _ => None,
    }
  }

  pub(crate) fn builtin_tag_name(&self) -> Option<&'static str> {
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
      Self::Unit => Some("unit"),
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
      Some(Calcit::Symbol { sym, .. }) => {
        let stripped = sym.trim_start_matches('\'');
        let n_quotes = sym.len() - stripped.len();
        if n_quotes > 0 {
          eprintln!(
            "[Error] Type variable `'{sym}` has excess leading quotes — expected a plain uppercase symbol like `'T`, got `'{sym}`"
          );
        }
        Some(Arc::from(stripped))
      }
      Some(Calcit::List(_)) => Self::parse_type_var_form(list.get(1)?),
      _ => None,
    }
  }

  fn generics_contains(generics: &[Arc<str>], name: &str) -> bool {
    let stripped = name.trim_start_matches('\'');
    generics.iter().any(|g| g.as_ref() == stripped)
  }

  fn extend_generics_scope(outer: &[Arc<str>], inner: &[Arc<str>]) -> Vec<Arc<str>> {
    let mut scope = outer.to_vec();
    for item in inner {
      if !scope.iter().any(|existing| existing.as_ref() == item.as_ref()) {
        scope.push(item.to_owned());
      }
    }
    scope
  }

  fn normalize_type_ref_name(name: &str) -> Arc<str> {
    Arc::from(name.trim_start_matches('\''))
  }

  fn extract_type_ref_name(form: &Calcit) -> Option<Arc<str>> {
    if let Some(name) = Self::parse_type_var_form(form) {
      return Some(name);
    }

    match form {
      Calcit::Symbol { sym, .. } => Some(Self::normalize_type_ref_name(sym)),
      Calcit::Import(import) => Some(Arc::from(format!("{}/{}", import.ns, import.def))),
      _ => None,
    }
  }

  fn type_ref_name_matches(name: &str, target: &str) -> bool {
    let left = name.trim_start_matches('\'').trim_start_matches(':');
    let right = target.trim_start_matches(':');
    left == right || left.rsplit('/').next().is_some_and(|segment| segment == right)
  }

  fn is_hint_fn_form(list: &CalcitList) -> bool {
    match list.first() {
      Some(Calcit::Syntax(CalcitSyntax::HintFn, _)) => true,
      Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "hint-fn" => true,
      _ => false,
    }
  }

  /// If `form` is a `hint-fn` expression, return its argument items (everything after the head).
  fn get_hint_fn_items(form: &Calcit) -> Option<&CalcitList> {
    let Calcit::List(list) = form else { return None };
    if !Self::is_hint_fn_form(list) {
      return None;
    }
    Some(list)
  }

  fn schema_key_name(form: &Calcit) -> Option<&str> {
    match form {
      Calcit::Tag(tag) => {
        let raw = tag.ref_str();
        Some(raw.strip_prefix(':').unwrap_or(raw))
      }
      Calcit::Symbol { sym, .. } => {
        let raw = sym.as_ref();
        Some(raw.strip_prefix(':').unwrap_or(raw))
      }
      Calcit::Str(text) => Some(text.as_ref()),
      _ => None,
    }
  }

  fn schema_key_matches(form: &Calcit, key: &str) -> bool {
    matches!(Self::schema_key_name(form), Some(name) if name == key)
  }

  fn is_schema_map_literal_head(form: &Calcit) -> bool {
    match form {
      Calcit::Symbol { sym, .. } if sym.as_ref() == "{}" => true,
      Calcit::Proc(CalcitProc::NativeMap) => true,
      Calcit::Import(CalcitImport { ns, def, .. }) if ns.as_ref() == CORE_NS && def.as_ref() == "{}" => true,
      _ => false,
    }
  }

  fn extract_schema_value_single<'a>(form: &'a Calcit, key: &str) -> Option<&'a Calcit> {
    match form {
      Calcit::Map(xs) => {
        for (entry_key, value) in xs {
          if Self::schema_key_matches(entry_key, key) {
            return Some(value);
          }
        }
        None
      }
      Calcit::List(xs) => {
        if !matches!(xs.first(), Some(head) if Self::is_schema_map_literal_head(head)) {
          return None;
        }

        for entry in xs.iter().skip(1) {
          let Calcit::List(pair) = entry else {
            continue;
          };
          if pair.len() < 2 {
            continue;
          }
          let Some(entry_key) = pair.get(0) else {
            continue;
          };
          let Some(value) = pair.get(1) else {
            continue;
          };
          if Self::schema_key_matches(entry_key, key) {
            return Some(value);
          }
        }
        None
      }
      _ => None,
    }
  }

  fn collect_fn_schema_fields<'a>(form: &'a Calcit) -> FnSchemaFields<'a> {
    let mut fields = FnSchemaFields::default();

    let mut visit_pair = |key: &'a Calcit, value: &'a Calcit| {
      let Some(key_name) = Self::schema_key_name(key) else {
        return;
      };
      match key_name {
        "generics" => {
          fields.has_any = true;
          if fields.generics.is_none() {
            fields.generics = Some(value);
          }
        }
        "where" => {
          fields.has_any = true;
          if fields.where_clause.is_none() {
            fields.where_clause = Some(value);
          }
        }
        "args" => {
          fields.has_any = true;
          if fields.args.is_none() {
            fields.args = Some(value);
          }
        }
        "return" => {
          fields.has_any = true;
          if fields.returns.is_none() {
            fields.returns = Some(value);
          }
        }
        "rest" => {
          fields.has_any = true;
          if fields.rest.is_none() {
            fields.rest = Some(value);
          }
        }
        "kind" => {
          fields.has_any = true;
          if fields.kind.is_none() {
            fields.kind = Some(value);
          }
        }
        _ => {}
      }
    };

    match form {
      Calcit::Map(xs) => {
        for (key, value) in xs {
          visit_pair(key, value);
        }
      }
      Calcit::List(xs) => {
        if !matches!(xs.first(), Some(head) if Self::is_schema_map_literal_head(head)) {
          return FnSchemaFields::default();
        }
        for entry in xs.iter().skip(1) {
          let Calcit::List(pair) = entry else {
            continue;
          };
          let Some(key) = pair.get(0) else {
            continue;
          };
          let Some(value) = pair.get(1) else {
            continue;
          };
          visit_pair(key, value);
        }
      }
      _ => return FnSchemaFields::default(),
    }

    fields
  }

  pub fn extract_return_type_from_hint_form(form: &Calcit) -> Option<Arc<CalcitTypeAnnotation>> {
    let generics = Self::extract_generics_from_hint_form(form).unwrap_or_default();
    let items = Self::get_hint_fn_items(form)?;
    for item in items.iter().skip(1) {
      if let Some(type_expr) = Self::extract_schema_value_single(item, "return") {
        return Some(CalcitTypeAnnotation::parse_type_annotation_form_with_generics(
          type_expr,
          generics.as_slice(),
        ));
      }
    }
    None
  }

  pub fn extract_generics_from_hint_form(form: &Calcit) -> Option<Vec<Arc<str>>> {
    let items = Self::get_hint_fn_items(form)?;
    for item in items.iter().skip(1) {
      if let Some(value) = Self::extract_schema_value_single(item, "generics") {
        if let Some(vars) = Self::parse_generics_list(value) {
          return Some(vars);
        }
      }
    }
    None
  }

  /// Check if the given Calcit value is the `[]` list constructor head.
  fn is_args_list_head(form: &Calcit) -> bool {
    match form {
      Calcit::Symbol { sym, .. } => sym.as_ref() == "[]",
      Calcit::Proc(CalcitProc::List) => true,
      Calcit::Import(CalcitImport { ns, def, .. }) => ns.as_ref() == CORE_NS && def.as_ref() == "[]",
      _ => false,
    }
  }

  /// Parse a schema `:args` value into a positional vec of type annotations.
  ///
  /// The `:args` form looks like `([] :type1 :type2 ...)` where `[]` is the list constructor head.
  fn parse_schema_args_types(form: &Calcit, count: usize, generics: &[Arc<str>]) -> Vec<Arc<CalcitTypeAnnotation>> {
    let mut result = vec![DYNAMIC_TYPE.clone(); count];

    let Calcit::List(xs) = form else {
      return result;
    };

    // Skip the `[]` list constructor head if present
    let start = if xs.first().map(Self::is_args_list_head).unwrap_or(false) {
      1
    } else {
      0
    };

    for (idx, type_form) in xs.iter().skip(start).enumerate() {
      if idx >= count {
        break;
      }
      result[idx] = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(type_form, generics);
    }
    result
  }

  fn parse_schema_args_list(form: &Calcit, generics: &[Arc<str>], strict_named_refs: bool) -> Vec<Arc<CalcitTypeAnnotation>> {
    let Calcit::List(xs) = form else {
      return vec![];
    };

    let start = if xs.first().map(Self::is_args_list_head).unwrap_or(false) {
      1
    } else {
      0
    };

    xs.iter()
      .skip(start)
      .map(|item| Self::parse_type_annotation_form_inner(item, generics, strict_named_refs))
      .collect()
  }

  fn parse_where_bound_name(form: &Calcit, generics: &[Arc<str>]) -> Option<Arc<str>> {
    let name = match form {
      Calcit::Symbol { sym, .. } => Arc::from(sym.trim_start_matches('\'')),
      _ => Self::parse_type_var_form(form)?,
    };

    if Self::generics_contains(generics, name.as_ref()) {
      Some(name)
    } else {
      None
    }
  }

  fn parse_trait_bounds_value(form: &Calcit, generics: &[Arc<str>], strict_named_refs: bool) -> Option<Vec<Arc<CalcitTrait>>> {
    if let Calcit::List(items) = form {
      let start = if items.first().map(Self::is_args_list_head).unwrap_or(false) {
        1
      } else {
        0
      };
      let mut traits: Vec<Arc<CalcitTrait>> = vec![];
      for item in items.iter().skip(start) {
        let parsed = Self::parse_type_annotation_form_inner(item, generics, strict_named_refs);
        match parsed.as_ref() {
          CalcitTypeAnnotation::Trait(trait_def) => traits.push(trait_def.clone()),
          _ => return None,
        }
      }
      if !traits.is_empty() {
        return Some(traits);
      }
    }

    match Self::parse_type_annotation_form_inner(form, generics, strict_named_refs).as_ref() {
      CalcitTypeAnnotation::Trait(trait_def) => Some(vec![trait_def.clone()]),
      _ => None,
    }
  }

  fn parse_where_bounds_form(form: &Calcit, generics: &[Arc<str>], strict_named_refs: bool) -> Vec<CalcitGenericBound> {
    let mut bounds: Vec<CalcitGenericBound> = vec![];

    let mut visit_pair = |key: &Calcit, value: &Calcit| {
      let Some(name) = Self::parse_where_bound_name(key, generics) else {
        return;
      };
      let Some(traits) = Self::parse_trait_bounds_value(value, generics, strict_named_refs) else {
        return;
      };
      if traits.is_empty() {
        return;
      }
      bounds.push(CalcitGenericBound {
        name,
        traits: Arc::new(traits),
      });
    };

    match form {
      Calcit::Map(xs) => {
        for (key, value) in xs {
          visit_pair(key, value);
        }
      }
      Calcit::List(xs) => {
        if !matches!(xs.first(), Some(head) if Self::is_schema_map_literal_head(head)) {
          return vec![];
        }
        for entry in xs.iter().skip(1) {
          let Calcit::List(pair) = entry else {
            continue;
          };
          let (Some(key), Some(value)) = (pair.get(0), pair.get(1)) else {
            continue;
          };
          visit_pair(key, value);
        }
      }
      _ => return vec![],
    }

    bounds.sort_by(|a, b| a.name.cmp(&b.name));
    bounds
  }

  fn parse_where_bounds_from_edn(form: &Edn, generics: &[Arc<str>]) -> Option<Vec<CalcitGenericBound>> {
    let Edn::Map(map) = form else {
      return None;
    };

    let parse_trait_from_edn = |value: &Edn| -> Option<Arc<CalcitTrait>> {
      match value {
        Edn::Symbol(sym) if !Self::generics_contains(generics, sym.as_ref()) => Some(Arc::new(CalcitTrait::new(
          EdnTag::new(sym.trim_start_matches('\'')),
          vec![],
          vec![],
        ))),
        Edn::Tag(tag) => Some(Arc::new(CalcitTrait::new(EdnTag::new(tag.ref_str()), vec![], vec![]))),
        _ => {
          let parsed = Self::parse_type_annotation_form_inner(&Self::edn_type_to_calcit(value), generics, true);
          match parsed.as_ref() {
            CalcitTypeAnnotation::Trait(trait_def) => Some(trait_def.clone()),
            _ => None,
          }
        }
      }
    };

    let mut bounds: Vec<CalcitGenericBound> = vec![];
    for (key, value) in &map.0 {
      let name = match key {
        Edn::Symbol(sym) => {
          let trimmed = sym.trim_start_matches('\'');
          if Self::generics_contains(generics, trimmed) {
            Arc::from(trimmed)
          } else {
            continue;
          }
        }
        _ => continue,
      };

      let traits = match value {
        Edn::List(xs) => {
          let mut traits: Vec<Arc<CalcitTrait>> = vec![];
          for item in &xs.0 {
            let trait_def = parse_trait_from_edn(item)?;
            traits.push(trait_def);
          }
          traits
        }
        _ => {
          let trait_def = parse_trait_from_edn(value)?;
          vec![trait_def]
        }
      };

      if traits.is_empty() {
        continue;
      }

      bounds.push(CalcitGenericBound {
        name,
        traits: Arc::new(traits),
      });
    }

    bounds.sort_by(|a, b| a.name.cmp(&b.name));
    Some(bounds)
  }

  pub fn extract_where_bounds_from_hint_form(form: &Calcit) -> Option<Vec<CalcitGenericBound>> {
    let generics = Self::extract_generics_from_hint_form(form).unwrap_or_default();
    let items = Self::get_hint_fn_items(form)?;
    for item in items.iter().skip(1) {
      if let Some(where_form) = Self::extract_schema_value_single(item, "where") {
        let bounds = Self::parse_where_bounds_form(where_form, generics.as_slice(), true);
        return if bounds.is_empty() { None } else { Some(bounds) };
      }
    }
    None
  }

  fn collect_malformed_fn_schema_values(form: &Calcit) -> Vec<&Calcit> {
    match form {
      Calcit::Map(xs) => xs
        .iter()
        .filter_map(|(key, value)| if matches!(key, Calcit::Nil) { Some(value) } else { None })
        .collect(),
      Calcit::List(xs) if matches!(xs.first(), Some(head) if Self::is_schema_map_literal_head(head)) => xs
        .iter()
        .skip(1)
        .filter_map(|entry| {
          let Calcit::List(pair) = entry else {
            return None;
          };
          match (pair.get(0), pair.get(1)) {
            (Some(Calcit::Nil), Some(value)) => Some(value),
            _ => None,
          }
        })
        .collect(),
      _ => vec![],
    }
  }

  fn infer_malformed_fn_schema(form: &Calcit, generics: &[Arc<str>], strict_named_refs: bool) -> Option<Arc<CalcitTypeAnnotation>> {
    let anonymous_values = Self::collect_malformed_fn_schema_values(form);

    if anonymous_values.is_empty() {
      return match form {
        Calcit::Map(xs) if xs.is_empty() => Some(Arc::new(CalcitTypeAnnotation::DynFn)),
        Calcit::List(xs) if matches!(xs.first(), Some(head) if Self::is_schema_map_literal_head(head)) && xs.len() == 1 => {
          Some(Arc::new(CalcitTypeAnnotation::DynFn))
        }
        _ => None,
      };
    }

    let _ = (generics, strict_named_refs);
    Some(Arc::new(CalcitTypeAnnotation::DynFn))
  }

  fn parse_fn_annotation_from_schema_form(
    form: &Calcit,
    generics: &[Arc<str>],
    strict_named_refs: bool,
  ) -> Option<Arc<CalcitTypeAnnotation>> {
    let fields = Self::collect_fn_schema_fields(form);
    if !fields.has_any {
      return Self::infer_malformed_fn_schema(form, generics, strict_named_refs);
    }

    let local_generics = fields.generics.and_then(Self::parse_generics_list).unwrap_or_default();
    let scope = Self::extend_generics_scope(generics, local_generics.as_slice());
    let where_bounds = fields
      .where_clause
      .map(|item| Self::parse_where_bounds_form(item, scope.as_slice(), strict_named_refs))
      .unwrap_or_default();
    let arg_types = fields
      .args
      .map(|args_form| Self::parse_schema_args_list(args_form, scope.as_slice(), strict_named_refs))
      .unwrap_or_default();
    let return_type = fields
      .returns
      .map(|item| Self::parse_type_annotation_form_inner(item, scope.as_slice(), strict_named_refs))
      .unwrap_or_else(|| Arc::new(Self::Dynamic));
    let rest_type = fields
      .rest
      .map(|item| Self::parse_type_annotation_form_inner(item, scope.as_slice(), strict_named_refs));
    let fn_kind = match fields.kind {
      Some(Calcit::Tag(tag)) if tag.ref_str() == "macro" => SchemaKind::Macro,
      Some(Calcit::Symbol { sym, .. }) if matches!(sym.as_ref(), ":macro" | "macro") => SchemaKind::Macro,
      _ => SchemaKind::Fn,
    };

    Some(Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(local_generics),
      where_bounds: Arc::new(where_bounds),
      arg_types,
      return_type,
      fn_kind,
      rest_type,
    }))))
  }

  /// Extract arg types from a schema hint-fn form, e.g. `(HintFn {:args ([] :number :fn) :return :number})`.
  ///
  /// Returns `None` if the hint-fn was not found or has no `:args` key. Used in `syntax::defn` as
  /// the highest-priority source for `CalcitFn.arg_types` (before `assert-type` body scanning).
  pub fn extract_arg_types_from_hint_form(form: &Calcit, params: &[Arc<str>]) -> Option<Vec<Arc<CalcitTypeAnnotation>>> {
    let generics = Self::extract_generics_from_hint_form(form).unwrap_or_default();
    let items = Self::get_hint_fn_items(form)?;
    for item in items.iter().skip(1) {
      if let Some(args_form) = Self::extract_schema_value_single(item, "args") {
        let types = Self::parse_schema_args_types(args_form, params.len(), generics.as_slice());
        return Some(types);
      }
    }
    None
  }

  /// Convert a type-annotation [`Edn`] value into its equivalent [`Calcit`] form so that
  /// [`Self::parse_type_annotation_form`] can be reused without duplicating its logic.
  /// Only the variants that appear inside schema type expressions need to be handled:
  /// tags, symbols, lists, and tuples.
  fn edn_type_to_calcit(form: &Edn) -> Calcit {
    match form {
      Edn::Nil => Calcit::Nil,
      Edn::Tag(t) => Calcit::Tag(t.clone()),
      Edn::Symbol(s) => Calcit::Symbol {
        sym: s.clone(),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from(CORE_NS),
          at_def: Arc::from("type-annotation"),
        }),
        location: None,
      },
      Edn::List(xs) => {
        let items: Vec<Calcit> = xs.0.iter().map(Self::edn_type_to_calcit).collect();
        Calcit::List(Arc::new(CalcitList::from(items.as_slice())))
      }
      Edn::Map(xs) => {
        let mut ys = rpds::HashTrieMap::new_sync();
        for (k, v) in &xs.0 {
          ys.insert_mut(Self::edn_type_to_calcit(k), Self::edn_type_to_calcit(v));
        }
        Calcit::Map(ys)
      }
      Edn::Tuple(view) => Calcit::Tuple(CalcitTuple {
        tag: Arc::new(Self::edn_type_to_calcit(view.tag.as_ref())),
        extra: view.extra.iter().map(Self::edn_type_to_calcit).collect(),
        sum_type: None,
      }),
      _ => Calcit::Nil,
    }
  }

  /// Parse a schema [`Edn`] map value (as stored in [`crate::snapshot::CodeEntry::schema`])
  /// directly into a [`CalcitFnTypeAnnotation`], without going through a Cirru/Calcit roundtrip.
  ///
  /// Accepts both a plain schema map and the wrapped top-level form `(:: :fn ({} ...))`
  /// or `(:: :macro ({} ...))`.
  /// Returns `None` only when the input does not look like a function schema at all.
  pub fn parse_fn_schema_from_edn(schema: &Edn) -> Option<CalcitFnTypeAnnotation> {
    let mut wrapped_kind: Option<SchemaKind> = None;
    let map = match schema {
      Edn::Map(map) => map,
      Edn::Tuple(view) if matches!(view.tag.as_ref(), Edn::Tag(tag) if matches!(tag.ref_str(), "fn" | "macro")) => {
        wrapped_kind = match view.tag.as_ref() {
          Edn::Tag(tag) if tag.ref_str() == "macro" => Some(SchemaKind::Macro),
          _ => Some(SchemaKind::Fn),
        };
        match view.extra.first() {
          Some(Edn::Map(map)) => map,
          _ => return None,
        }
      }
      _ => return None,
    };

    let has_schema_fields = ["kind", "args", "return", "generics", "where", "rest"]
      .iter()
      .any(|key| map.tag_get(key).is_some());
    if !has_schema_fields {
      return None;
    }

    let generics: Vec<Arc<str>> = match map.tag_get("generics") {
      None => vec![],
      Some(Edn::List(xs)) => xs
        .0
        .iter()
        .map(|x| match x {
          Edn::Symbol(s) if !s.starts_with('\'') => Some(Arc::from(s.as_ref())),
          _ => None,
        })
        .collect::<Option<Vec<_>>>()?,
      Some(_) => return None,
    };

    let arg_types: Vec<Arc<CalcitTypeAnnotation>> = map
      .tag_get("args")
      .and_then(|v| if let Edn::List(xs) = v { Some(xs) } else { None })
      .map(|xs| {
        xs.0
          .iter()
          .map(|x| Self::parse_type_annotation_form_with_generics(&Self::edn_type_to_calcit(x), generics.as_slice()))
          .collect()
      })
      .unwrap_or_default();

    let return_type = map
      .tag_get("return")
      .map(|v| Self::parse_type_annotation_form_with_generics(&Self::edn_type_to_calcit(v), generics.as_slice()))
      .unwrap_or_else(|| crate::calcit::DYNAMIC_TYPE.clone());
    let where_bounds = map
      .tag_get("where")
      .and_then(|value| Self::parse_where_bounds_from_edn(value, generics.as_slice()))
      .unwrap_or_default();

    let fn_kind = match map.tag_get("kind") {
      Some(Edn::Tag(t)) if t.ref_str() == "macro" => SchemaKind::Macro,
      _ => wrapped_kind.unwrap_or(SchemaKind::Fn),
    };
    let rest_type = map
      .tag_get("rest")
      .map(|v| Self::parse_type_annotation_form_with_generics(&Self::edn_type_to_calcit(v), generics.as_slice()));
    Some(CalcitFnTypeAnnotation {
      generics: Arc::new(generics),
      where_bounds: Arc::new(where_bounds),
      arg_types,
      return_type,
      fn_kind,
      rest_type,
    })
  }

  fn parse_generics_list(form: &Calcit) -> Option<Vec<Arc<str>>> {
    let Calcit::List(items) = form else {
      return None;
    };

    // Skip a leading `[]` list-constructor head so that `([] 'T 'U)` is accepted
    // as a generics list with two TypeVars.
    let start = if items.first().map(Self::is_args_list_head).unwrap_or(false) {
      1
    } else {
      0
    };

    let mut vars = Vec::with_capacity(items.len());
    for item in items.iter().skip(start) {
      if let Some(name) = Self::parse_type_var_form(item) {
        vars.push(name);
        continue;
      }
      if let Calcit::Symbol { sym, .. } = item {
        let stripped = sym.trim_start_matches('\'');
        let n_quotes = sym.len() - stripped.len();
        if n_quotes > 0 {
          eprintln!("[Error] Generic type variable `{sym}` has excess leading quotes — expected plain uppercase like `'T`");
        }
        vars.push(Arc::from(stripped));
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

      for i in 3..list.len() {
        if let Some(form) = list.get(i)
          && let Some(g) = Self::extract_generics_from_hint_form(form)
        {
          generics = g;
        }
      }

      // Scan body forms for available hints only; do not expand macros.
      for i in 3..list.len() {
        if let Some(form) = list.get(i) {
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
                let t = Self::parse_type_annotation_form_with_generics(type_form, generics.as_slice());
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
        where_bounds: Arc::new(vec![]),
        arg_types: final_arg_types,
        return_type,
        fn_kind: SchemaKind::Fn,
        rest_type: None,
      };
      return Some(signature.render_signature_brief());
    }
    None
  }

  /// Collect arg type hints for function parameters by scanning `assert-type` in body forms.
  ///
  /// This is intentionally different from return-type handling: return-type uses `hint-fn`, while
  /// arg types are sourced from `assert-type` inside function bodies. If no `assert-type` is found,
  /// the parameter stays `dynamic` and no checking occurs.
  pub fn collect_arg_type_hints_from_body(
    body_items: &[Calcit],
    params: &[Arc<str>],
    generics: &[Arc<str>],
  ) -> Vec<Arc<CalcitTypeAnnotation>> {
    let mut arg_types = vec![DYNAMIC_TYPE.clone(); params.len()];
    if params.is_empty() {
      return arg_types;
    }

    let mut param_index: std::collections::HashMap<Arc<str>, usize> = std::collections::HashMap::with_capacity(params.len());
    for (idx, sym) in params.iter().enumerate() {
      param_index.entry(sym.to_owned()).or_insert(idx);
    }

    for form in body_items {
      Self::scan_body_for_arg_types(form, &param_index, generics, &mut arg_types);
    }

    arg_types
  }

  /// Walk a form tree to find `(assert-type <param> <type>)` and map it to the param index.
  ///
  /// Unlike `parse_type_annotation_form`, this inspects raw body forms and ignores nested defn/defmacro.
  fn scan_body_for_arg_types(
    form: &Calcit,
    param_index: &std::collections::HashMap<Arc<str>, usize>,
    generics: &[Arc<str>],
    arg_types: &mut [Arc<CalcitTypeAnnotation>],
  ) {
    fn is_trait_annotation(ann: &CalcitTypeAnnotation) -> bool {
      matches!(ann, CalcitTypeAnnotation::Trait(_) | CalcitTypeAnnotation::TraitSet(_))
        || matches!(ann, CalcitTypeAnnotation::Optional(inner) if is_trait_annotation(inner.as_ref()))
    }

    fn is_dynamic_annotation(ann: &CalcitTypeAnnotation) -> bool {
      matches!(ann, CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn)
        || matches!(ann, CalcitTypeAnnotation::Optional(inner) if is_dynamic_annotation(inner.as_ref()))
    }

    fn is_concrete_annotation(ann: &CalcitTypeAnnotation) -> bool {
      !is_dynamic_annotation(ann) && !is_trait_annotation(ann)
    }

    let list = match form {
      Calcit::List(xs) => xs,
      _ => return,
    };

    if let Some((target, trait_forms)) = Self::extract_assert_traits_args(list) {
      let sym = match target {
        Calcit::Symbol { sym, .. } => sym.to_owned(),
        Calcit::Local(local) => local.sym.to_owned(),
        _ => return,
      };

      if let Some(&idx) = param_index.get(&sym) {
        if is_concrete_annotation(arg_types[idx].as_ref()) {
          return;
        }
        let mut traits: Vec<Arc<CalcitTrait>> = vec![];
        let mut non_trait: Option<Arc<CalcitTypeAnnotation>> = None;
        for form in trait_forms {
          let parsed = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(form, generics);
          match parsed.as_ref() {
            CalcitTypeAnnotation::Trait(trait_def) => traits.push(trait_def.to_owned()),
            _ => {
              if non_trait.is_none() {
                non_trait = Some(parsed);
              }
            }
          }
        }

        if !traits.is_empty() {
          if traits.len() == 1 && non_trait.is_none() {
            arg_types[idx] = Arc::new(CalcitTypeAnnotation::Trait(traits.remove(0)));
          } else {
            arg_types[idx] = Arc::new(CalcitTypeAnnotation::TraitSet(Arc::new(traits)));
          }
        } else if let Some(fallback) = non_trait {
          arg_types[idx] = fallback;
        }
      }
      return;
    }

    if let Some((target, type_expr)) = Self::extract_assert_type_args(list) {
      let sym = match target {
        Calcit::Symbol { sym, .. } => sym.to_owned(),
        Calcit::Local(local) => local.sym.to_owned(),
        _ => return,
      };

      if let Some(&idx) = param_index.get(&sym) {
        arg_types[idx] = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(type_expr, generics);
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
      Self::scan_body_for_arg_types(item, param_index, generics, arg_types);
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

  fn extract_assert_traits_args(list: &CalcitList) -> Option<(&Calcit, Vec<&Calcit>)> {
    match list.first() {
      Some(Calcit::Syntax(CalcitSyntax::AssertTraits, _)) => {}
      Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "assert-traits" => {}
      _ => return None,
    }

    let target = list.get(1)?;
    let mut trait_forms: Vec<&Calcit> = vec![];
    for item in list.iter().skip(2) {
      trait_forms.push(item);
    }
    if trait_forms.is_empty() {
      return None;
    }
    Some((target, trait_forms))
  }

  pub fn parse_type_annotation_form(form: &Calcit) -> Arc<CalcitTypeAnnotation> {
    Self::parse_type_annotation_form_inner(form, &[], false)
  }

  pub(crate) fn parse_type_annotation_form_with_generics(form: &Calcit, generics: &[Arc<str>]) -> Arc<CalcitTypeAnnotation> {
    Self::parse_type_annotation_form_inner(form, generics, true)
  }

  fn parse_type_annotation_form_inner(form: &Calcit, generics: &[Arc<str>], strict_named_refs: bool) -> Arc<CalcitTypeAnnotation> {
    let is_optional_tag = |tag: &EdnTag| tag.ref_str().trim_start_matches(':') == "optional";
    let is_list_tag = |tag: &EdnTag| tag.ref_str().trim_start_matches(':') == "list";
    let is_map_tag = |tag: &EdnTag| tag.ref_str().trim_start_matches(':') == "map";
    let is_set_tag = |tag: &EdnTag| tag.ref_str().trim_start_matches(':') == "set";
    let is_ref_tag = |tag: &EdnTag| tag.ref_str().trim_start_matches(':') == "ref";

    let parse_nested = |item: &Calcit| Self::parse_type_annotation_form_inner(item, generics, strict_named_refs);

    if matches!(form, Calcit::Nil) {
      return DYNAMIC_TYPE.clone();
    }

    if let Some(name) = Self::parse_type_var_form(form) {
      return if strict_named_refs && !Self::generics_contains(generics, &name) {
        Arc::new(CalcitTypeAnnotation::TypeRef(name, Arc::new(vec![])))
      } else {
        Arc::new(CalcitTypeAnnotation::TypeVar(name))
      };
    }

    if let Calcit::Symbol { sym, .. } = form {
      if sym.starts_with('\'') {
        let stripped = sym.trim_start_matches('\'');
        let n_quotes = sym.len() - stripped.len();
        if n_quotes > 1 {
          eprintln!("[Error] Type variable `{sym}` has excess leading quotes — expected a single-quoted uppercase symbol like `'T`");
        }
        return if strict_named_refs && !Self::generics_contains(generics, stripped) {
          Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from(stripped), Arc::new(vec![])))
        } else {
          Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from(stripped)))
        };
      }
      // Type slot reference: *name → TypeSlot(name)
      if sym.starts_with('*') {
        let slot_name = sym.trim_start_matches('*');
        if !slot_name.is_empty() {
          return Arc::new(CalcitTypeAnnotation::TypeSlot(Arc::from(slot_name)));
        }
      }
      if strict_named_refs && Self::generics_contains(generics, sym) {
        return Arc::new(CalcitTypeAnnotation::TypeVar(sym.to_owned()));
      }
    }

    if let Calcit::Tuple(tuple) = form {
      if let Some(struct_def) = resolve_struct_def(tuple.tag.as_ref()) {
        let args = tuple.extra.iter().map(parse_nested).collect::<Vec<_>>();
        return Arc::new(CalcitTypeAnnotation::Struct(Arc::new(struct_def), Arc::new(args)));
      }
      if let Calcit::Tag(tag) = tuple.tag.as_ref() {
        if is_optional_tag(tag) {
          if tuple.extra.len() != 1 {
            eprintln!("[Warn] :optional expects 1 argument, got {}", tuple.extra.len());
          }
          if let Some(inner_form) = tuple.extra.first() {
            return Arc::new(CalcitTypeAnnotation::Optional(parse_nested(inner_form)));
          }
        }
        if is_list_tag(tag) {
          if tuple.extra.len() > 1 {
            eprintln!("[Warn] :list expects at most 1 argument, got {}", tuple.extra.len());
          }
          if let Some(inner_form) = tuple.extra.first() {
            return Arc::new(CalcitTypeAnnotation::List(parse_nested(inner_form)));
          }
          return Arc::new(CalcitTypeAnnotation::List(Arc::new(Self::Dynamic)));
        }
        if is_map_tag(tag) {
          if tuple.extra.len() > 2 {
            eprintln!("[Warn] :map expects at most 2 arguments, got {}", tuple.extra.len());
          }
          let key_type = tuple.extra.first().map(parse_nested).unwrap_or_else(|| Arc::new(Self::Dynamic));
          let val_type = tuple.extra.get(1).map(parse_nested).unwrap_or_else(|| Arc::new(Self::Dynamic));
          return Arc::new(CalcitTypeAnnotation::Map(key_type, val_type));
        }
        if is_set_tag(tag) {
          if tuple.extra.len() > 1 {
            eprintln!("[Warn] :set expects at most 1 argument, got {}", tuple.extra.len());
          }
          if let Some(inner_form) = tuple.extra.first() {
            return Arc::new(CalcitTypeAnnotation::Set(parse_nested(inner_form)));
          }
          return Arc::new(CalcitTypeAnnotation::Set(Arc::new(Self::Dynamic)));
        }
        if is_ref_tag(tag) {
          if tuple.extra.len() > 1 {
            eprintln!("[Warn] :ref expects at most 1 argument, got {}", tuple.extra.len());
          }
          if let Some(inner_form) = tuple.extra.first() {
            return Arc::new(CalcitTypeAnnotation::Ref(parse_nested(inner_form)));
          }
          return Arc::new(CalcitTypeAnnotation::Ref(Arc::new(Self::Dynamic)));
        }
        if tag.ref_str().trim_start_matches(':') == "fn" {
          if let Some(schema_form) = tuple.extra.first()
            && let Some(parsed) = Self::parse_fn_annotation_from_schema_form(schema_form, generics, strict_named_refs)
          {
            return parsed;
          }
          if tuple.extra.is_empty() {
            return Arc::new(CalcitTypeAnnotation::DynFn);
          }
          emit_legacy_fn_type_syntax_warning(":: :fn $ {} ...", form);
          return Arc::new(CalcitTypeAnnotation::DynFn);
        }
      }

      let base = Self::parse_type_annotation_form_inner(tuple.tag.as_ref(), generics, strict_named_refs);
      let args = tuple.extra.iter().map(parse_nested).collect::<Vec<_>>();
      match base.as_ref() {
        CalcitTypeAnnotation::Struct(struct_def, _) => {
          return Arc::new(CalcitTypeAnnotation::Struct(struct_def.clone(), Arc::new(args)));
        }
        CalcitTypeAnnotation::Enum(enum_def, _) => {
          return Arc::new(CalcitTypeAnnotation::Enum(enum_def.clone(), Arc::new(args)));
        }
        CalcitTypeAnnotation::TypeRef(name, _) if strict_named_refs => {
          return Arc::new(CalcitTypeAnnotation::TypeRef(name.clone(), Arc::new(args)));
        }
        _ => {}
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
            return Arc::new(CalcitTypeAnnotation::Optional(parse_nested(inner_form)));
          }
        }
        if is_list_tag(tag) {
          if xs.len() > 2 {
            eprintln!("[Warn] :list expects at most 1 argument, got {}", xs.len() as i64 - 1);
          }
          if let Some(inner_form) = xs.get(1) {
            return Arc::new(CalcitTypeAnnotation::List(parse_nested(inner_form)));
          }
          return Arc::new(CalcitTypeAnnotation::List(Arc::new(Self::Dynamic)));
        }
        if is_map_tag(tag) {
          if xs.len() > 3 {
            eprintln!("[Warn] :map expects at most 2 arguments, got {}", xs.len() as i64 - 1);
          }
          let key_type = xs.get(1).map(parse_nested).unwrap_or_else(|| Arc::new(Self::Dynamic));
          let val_type = xs.get(2).map(parse_nested).unwrap_or_else(|| Arc::new(Self::Dynamic));
          return Arc::new(CalcitTypeAnnotation::Map(key_type, val_type));
        }
        if is_set_tag(tag) {
          if xs.len() > 2 {
            eprintln!("[Warn] :set expects at most 1 argument, got {}", xs.len() as i64 - 1);
          }
          if let Some(inner_form) = xs.get(1) {
            return Arc::new(CalcitTypeAnnotation::Set(parse_nested(inner_form)));
          }
          return Arc::new(CalcitTypeAnnotation::Set(Arc::new(Self::Dynamic)));
        }
        if is_ref_tag(tag) {
          if xs.len() > 2 {
            eprintln!("[Warn] :ref expects at most 1 argument, got {}", xs.len() as i64 - 1);
          }
          if let Some(inner_form) = xs.get(1) {
            return Arc::new(CalcitTypeAnnotation::Ref(parse_nested(inner_form)));
          }
          return Arc::new(CalcitTypeAnnotation::Ref(Arc::new(Self::Dynamic)));
        }
        if tag_name == "fn" {
          if let Some(schema_form) = xs.get(1)
            && let Some(parsed) = Self::parse_fn_annotation_from_schema_form(schema_form, generics, strict_named_refs)
          {
            return parsed;
          }
          if xs.len() == 1 {
            return Arc::new(CalcitTypeAnnotation::DynFn);
          }
          emit_legacy_fn_type_syntax_warning("(:fn {} ...)", form);
          return Arc::new(CalcitTypeAnnotation::DynFn);
        }
      }

      let is_tuple_constructor = match xs.first() {
        Some(Calcit::Proc(CalcitProc::NativeTuple)) => true,
        Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "::" => true,
        _ => false,
      };

      if is_tuple_constructor {
        if xs.len() == 3
          && let (Some(Calcit::Tag(marker)), Some(inner_form)) = (xs.get(1), xs.get(2))
          && marker.ref_str().trim_start_matches(':') == "&"
        {
          return Arc::new(CalcitTypeAnnotation::Variadic(parse_nested(inner_form)));
        }

        if let Some(Calcit::Tag(tag)) = xs.get(1) {
          if is_optional_tag(tag) {
            if xs.len() != 3 {
              eprintln!("[Warn] :optional expects 1 argument, got {}", xs.len() as i64 - 2);
            }
            if let Some(inner_form) = xs.get(2) {
              return Arc::new(CalcitTypeAnnotation::Optional(parse_nested(inner_form)));
            }
          }
        }

        if let Some(Calcit::Tag(tag)) = xs.get(1) {
          let tag_name = tag.ref_str().trim_start_matches(':');
          if tag_name == "record" {
            if xs.len() < 3 {
              eprintln!("[Warn] :: :record expects struct name, got {}", xs.len() as i64 - 2);
            } else if let Some(struct_def) = resolve_struct_annotation(xs.get(2).unwrap(), xs.get(3)) {
              return Arc::new(CalcitTypeAnnotation::Struct(Arc::new(struct_def), Arc::new(vec![])));
            }
          }
          if tag_name == "tuple" {
            if xs.len() < 3 {
              eprintln!("[Warn] :: :tuple expects enum name, got {}", xs.len() as i64 - 2);
            } else if let Some(enum_def) = resolve_enum_annotation(xs.get(2).unwrap(), xs.get(3)) {
              return Arc::new(CalcitTypeAnnotation::Enum(Arc::new(enum_def), Arc::new(vec![])));
            }
          }
          if tag_name == "list" {
            if let Some(inner_form) = xs.get(2) {
              return Arc::new(CalcitTypeAnnotation::List(parse_nested(inner_form)));
            }
            return Arc::new(CalcitTypeAnnotation::List(Arc::new(Self::Dynamic)));
          }
          if tag_name == "map" {
            let key_type = xs.get(2).map(parse_nested).unwrap_or_else(|| Arc::new(Self::Dynamic));
            let val_type = xs.get(3).map(parse_nested).unwrap_or_else(|| Arc::new(Self::Dynamic));
            return Arc::new(CalcitTypeAnnotation::Map(key_type, val_type));
          }
          if tag_name == "set" {
            if let Some(inner_form) = xs.get(2) {
              return Arc::new(CalcitTypeAnnotation::Set(parse_nested(inner_form)));
            }
            return Arc::new(CalcitTypeAnnotation::Set(Arc::new(Self::Dynamic)));
          }
          if tag_name == "ref" {
            if let Some(inner_form) = xs.get(2) {
              return Arc::new(CalcitTypeAnnotation::Ref(parse_nested(inner_form)));
            }
            return Arc::new(CalcitTypeAnnotation::Ref(Arc::new(Self::Dynamic)));
          }
          if tag_name == "fn" {
            if let Some(schema_form) = xs.get(2)
              && let Some(parsed) = Self::parse_fn_annotation_from_schema_form(schema_form, generics, strict_named_refs)
            {
              return parsed;
            }
            if xs.len() == 2 {
              return Arc::new(CalcitTypeAnnotation::DynFn);
            }
            emit_legacy_fn_type_syntax_warning(":: :fn $ {} ...", form);
            return Arc::new(CalcitTypeAnnotation::DynFn);
          }
        }

        if let Some(base_form) = xs.get(1) {
          let base = Self::parse_type_annotation_form_inner(base_form, generics, strict_named_refs);
          let args = xs
            .iter()
            .skip(2)
            .map(|item| Self::parse_type_annotation_form_inner(item, generics, strict_named_refs))
            .collect::<Vec<_>>();
          match base.as_ref() {
            CalcitTypeAnnotation::Struct(struct_def, _) => {
              return Arc::new(CalcitTypeAnnotation::Struct(struct_def.clone(), Arc::new(args)));
            }
            CalcitTypeAnnotation::Enum(enum_def, _) => {
              return Arc::new(CalcitTypeAnnotation::Enum(enum_def.clone(), Arc::new(args)));
            }
            CalcitTypeAnnotation::TypeRef(name, _) if strict_named_refs => {
              return Arc::new(CalcitTypeAnnotation::TypeRef(name.clone(), Arc::new(args)));
            }
            _ => {}
          }
        }
      }
    }

    if let Some(resolved) = resolve_calcit_value(form) {
      match resolved {
        Calcit::Trait(trait_def) => return Arc::new(CalcitTypeAnnotation::Trait(Arc::new(trait_def))),
        Calcit::Struct(struct_def) if !strict_named_refs => {
          return Arc::new(CalcitTypeAnnotation::Struct(Arc::new(struct_def), Arc::new(vec![])));
        }
        Calcit::Enum(enum_def) if !strict_named_refs => {
          return Arc::new(CalcitTypeAnnotation::Enum(Arc::new(enum_def), Arc::new(vec![])));
        }
        _ => {}
      }
    }

    if strict_named_refs && let Some(name) = Self::extract_type_ref_name(form) {
      return Arc::new(CalcitTypeAnnotation::TypeRef(name, Arc::new(vec![])));
    }

    Arc::new(CalcitTypeAnnotation::from_calcit(form))
  }

  /// Render a concise representation used in warnings or logs
  pub fn to_brief_string(&self) -> String {
    if let Some(tag) = self.builtin_tag_name() {
      return format!(":{tag}");
    }

    match self {
      Self::Fn(signature) => signature.render_signature_brief(),
      Self::Variadic(inner) => format!("&{}", inner.to_brief_string()),
      Self::List(inner) => format!("list<{}>", inner.to_brief_string()),
      Self::Map(k, v) => format!("map<{},{}>", k.to_brief_string(), v.to_brief_string()),
      Self::Set(inner) => format!("set<{}>", inner.to_brief_string()),
      Self::Ref(inner) => format!("ref<{}>", inner.to_brief_string()),
      Self::Custom(inner) => format!("{inner}"),
      Self::Optional(inner) => format!("{}?", inner.to_brief_string()),
      Self::Struct(base, args) => {
        if args.is_empty() {
          format!("struct {}", base.name)
        } else {
          let rendered = args.iter().map(|t| t.to_brief_string()).collect::<Vec<_>>().join(", ");
          format!("struct {}<{}>", base.name, rendered)
        }
      }
      Self::Trait(trait_def) => format!("trait {}", trait_def.name),
      Self::TraitSet(traits) => {
        let rendered = traits.iter().map(|t| t.name.to_string()).collect::<Vec<_>>().join(" ");
        format!("traits {rendered}")
      }
      Self::TypeVar(name) => format!("'{name}"),
      Self::TypeRef(name, args) => {
        if args.is_empty() {
          format!("'{name}")
        } else {
          let rendered = args.iter().map(|t| t.to_brief_string()).collect::<Vec<_>>().join(", ");
          format!("'{name}<{rendered}>")
        }
      }
      Self::Enum(enum_def, args) => {
        if args.is_empty() {
          format!("enum {}", enum_def.name())
        } else {
          let rendered = args.iter().map(|t| t.to_brief_string()).collect::<Vec<_>>().join(", ");
          format!("enum {}<{}>", enum_def.name(), rendered)
        }
      }
      Self::Record(struct_def) => format!("struct {}", struct_def.name),
      Self::Tuple(enum_def) => format!("enum {}", enum_def.name()),
      Self::Dynamic => "dynamic".to_string(),
      Self::TypeSlot(name) => format!("type-slot({name})"),
      _ => "unknown".to_string(),
    }
  }

  /// Substitute all `TypeVar` occurrences with their bound types from `bindings`.
  /// Returns a new annotation with variables resolved; unbound variables remain as-is.
  pub fn substitute_type_vars(&self, bindings: &TypeBindings) -> Arc<CalcitTypeAnnotation> {
    match self {
      Self::TypeVar(name) => bindings.get(name).cloned().unwrap_or_else(|| Arc::new(self.clone())),
      Self::TypeRef(name, args) => {
        let new_args: Vec<_> = args.iter().map(|a| a.substitute_type_vars(bindings)).collect();
        Arc::new(Self::TypeRef(name.clone(), Arc::new(new_args)))
      }
      Self::List(inner) => Arc::new(Self::List(inner.substitute_type_vars(bindings))),
      Self::Map(k, v) => Arc::new(Self::Map(k.substitute_type_vars(bindings), v.substitute_type_vars(bindings))),
      Self::Set(inner) => Arc::new(Self::Set(inner.substitute_type_vars(bindings))),
      Self::Ref(inner) => Arc::new(Self::Ref(inner.substitute_type_vars(bindings))),
      Self::Optional(inner) => Arc::new(Self::Optional(inner.substitute_type_vars(bindings))),
      Self::Variadic(inner) => Arc::new(Self::Variadic(inner.substitute_type_vars(bindings))),
      Self::Fn(sig) => {
        let new_args = sig.arg_types.iter().map(|a| a.substitute_type_vars(bindings)).collect();
        let new_ret = sig.return_type.substitute_type_vars(bindings);
        let new_rest = sig.rest_type.as_ref().map(|r| r.substitute_type_vars(bindings));
        Arc::new(Self::Fn(Arc::new(CalcitFnTypeAnnotation {
          generics: sig.generics.clone(),
          where_bounds: sig.where_bounds.clone(),
          arg_types: new_args,
          return_type: new_ret,
          fn_kind: sig.fn_kind,
          rest_type: new_rest,
        })))
      }
      Self::Struct(base, args) => {
        let new_args: Vec<_> = args.iter().map(|a| a.substitute_type_vars(bindings)).collect();
        Arc::new(Self::Struct(base.clone(), Arc::new(new_args)))
      }
      Self::Enum(base, args) => {
        let new_args: Vec<_> = args.iter().map(|a| a.substitute_type_vars(bindings)).collect();
        Arc::new(Self::Enum(base.clone(), Arc::new(new_args)))
      }
      // Leaf types: no TypeVars inside
      _ => Arc::new(self.clone()),
    }
  }

  /// Check whether this annotation contains any `TypeVar`.
  pub fn contains_type_var(&self) -> bool {
    match self {
      Self::TypeVar(_) => true,
      Self::TypeRef(_, args) => args.iter().any(|a| a.contains_type_var()),
      Self::List(inner) | Self::Set(inner) | Self::Ref(inner) | Self::Optional(inner) | Self::Variadic(inner) => {
        inner.contains_type_var()
      }
      Self::Map(k, v) => k.contains_type_var() || v.contains_type_var(),
      Self::Fn(sig) => sig.arg_types.iter().any(|a| a.contains_type_var()) || sig.return_type.contains_type_var(),
      Self::Struct(_, args) | Self::Enum(_, args) => args.iter().any(|a| a.contains_type_var()),
      _ => false,
    }
  }

  /// Try to resolve this type annotation to a concrete `CalcitStruct` definition.
  /// Works for `Struct(def, _)`, `Record(def)`, and `TypeRef("ns/name", _)` that can be
  /// looked up from the program registry.
  pub fn resolve_to_struct(&self) -> Option<CalcitStruct> {
    self.resolve_to_struct_with_ref().map(|(s, _)| s)
  }

  /// Resolve to struct, also returning the (ns, def) path when available from a TypeRef.
  /// The path can be used to construct an Import reference for JS codegen compatibility.
  #[allow(clippy::type_complexity)]
  pub fn resolve_to_struct_with_ref(&self) -> Option<(CalcitStruct, Option<(Arc<str>, Arc<str>)>)> {
    match self {
      Self::Struct(base, _) => Some((base.as_ref().clone(), None)),
      Self::Record(base) => Some((base.as_ref().clone(), None)),
      Self::TypeRef(name, _) => {
        // TypeRef name may be "ns/def" or just "def" — try to split on '/'
        let stripped = name.trim_start_matches('\'').trim_start_matches(':');
        if let Some((ns, def)) = stripped.rsplit_once('/') {
          resolve_struct_from_program(ns, def).map(|s| (s, Some((Arc::from(ns), Arc::from(def)))))
        } else {
          None
        }
      }
      Self::Optional(inner) => inner.resolve_to_struct_with_ref(),
      _ => None,
    }
  }

  /// Try to resolve this type annotation to a concrete `CalcitEnum` definition.
  /// Works for `Enum(def, _)`, `Tuple(def)`, and `TypeRef("ns/name", _)` that can be
  /// looked up from the program registry.
  pub fn resolve_to_enum(&self) -> Option<CalcitEnum> {
    self.resolve_to_enum_with_ref().map(|(e, _)| e)
  }

  /// Resolve to enum, also returning the (ns, def) path when available from a TypeRef.
  /// The path can be used to construct an Import reference for JS codegen compatibility.
  #[allow(clippy::type_complexity)]
  pub fn resolve_to_enum_with_ref(&self) -> Option<(CalcitEnum, Option<(Arc<str>, Arc<str>)>)> {
    match self {
      Self::Enum(base, _) => Some((base.as_ref().clone(), None)),
      Self::Tuple(base) => Some((base.as_ref().clone(), None)),
      Self::TypeRef(name, _) => {
        let stripped = name.trim_start_matches('\'').trim_start_matches(':');
        if let Some((ns, def)) = stripped.rsplit_once('/') {
          resolve_enum_from_program(ns, def).map(|e| (e, Some((Arc::from(ns), Arc::from(def)))))
        } else {
          None
        }
      }
      Self::Optional(inner) => inner.resolve_to_enum_with_ref(),
      Self::TypeSlot(name) => resolve_type_slot(name).and_then(|bound| bound.resolve_to_enum_with_ref()),
      _ => None,
    }
  }

  /// Resolve this type annotation to a `Fn` type, unwrapping Optional/TypeRef/TypeSlot layers.
  pub fn resolve_to_fn(&self) -> Option<Arc<CalcitFnTypeAnnotation>> {
    match self {
      Self::Fn(fn_annot) => Some(fn_annot.clone()),
      Self::Optional(inner) => inner.resolve_to_fn(),
      Self::TypeRef(name, _) => {
        let stripped = name.trim_start_matches('\'').trim_start_matches(':');
        resolve_type_ref_as_schema(stripped).and_then(|schema| schema.resolve_to_fn())
      }
      Self::TypeSlot(name) => resolve_type_slot(name).and_then(|bound| bound.resolve_to_fn()),
      _ => None,
    }
  }

  fn core_impl_list_symbol(&self) -> Option<&'static str> {
    match self {
      Self::List(_) => Some("&core-list-impls"),
      Self::String => Some("&core-string-impls"),
      Self::Map(_, _) => Some("&core-map-impls"),
      Self::Set(_) => Some("&core-set-impls"),
      Self::Number => Some("&core-number-impls"),
      Self::DynFn | Self::Fn(_) => Some("&core-fn-impls"),
      Self::Optional(inner) => inner.core_impl_list_symbol(),
      Self::TypeRef(name, _) => resolve_type_ref_as_schema(name).and_then(|schema| schema.core_impl_list_symbol()),
      Self::TypeSlot(name) => resolve_type_slot(name).and_then(|bound| bound.core_impl_list_symbol()),
      _ => None,
    }
  }

  fn resolve_impl_from_value(value: &Calcit) -> Option<CalcitImpl> {
    match value {
      Calcit::Impl(imp) => Some(imp.clone()),
      Calcit::Import(import) => resolve_calcit_value(&Calcit::Import(import.clone()))
        .and_then(|resolved| match resolved {
          Calcit::Impl(imp) => Some(imp),
          _ => None,
        })
        .or_else(|| {
          lookup_def_code_registered(import.ns.as_ref(), import.def.as_ref())
            .and_then(|code| Self::extract_def_value_from_code(&code))
            .and_then(|resolved| match resolved {
              Calcit::Impl(imp) => Some(imp),
              _ => None,
            })
        }),
      Calcit::Symbol { info, sym, .. } => resolve_calcit_value(value)
        .and_then(|resolved| match resolved {
          Calcit::Impl(imp) => Some(imp),
          _ => None,
        })
        .or_else(|| {
          lookup_def_code_registered(info.at_ns.as_ref(), sym)
            .and_then(|code| Self::extract_def_value_from_code(&code))
            .and_then(|resolved| match resolved {
              Calcit::Impl(imp) => Some(imp),
              _ => None,
            })
        }),
      _ => None,
    }
  }

  fn collect_impls_from_value(value: &Calcit) -> Option<Vec<Arc<CalcitImpl>>> {
    match value {
      Calcit::Impl(_) | Calcit::Import(_) | Calcit::Symbol { .. } => {
        Self::resolve_impl_from_value(value).map(|imp| vec![Arc::new(imp)])
      }
      Calcit::List(items) => {
        let start = if items.first().is_some_and(Self::is_args_list_head) { 1 } else { 0 };
        let mut impls: Vec<Arc<CalcitImpl>> = Vec::with_capacity(items.len().saturating_sub(start));
        for item in items.iter().skip(start) {
          if let Some(imp) = Self::resolve_impl_from_value(item) {
            impls.push(Arc::new(imp));
          }
        }
        if impls.is_empty() { None } else { Some(impls) }
      }
      _ => None,
    }
  }

  fn extract_def_value_from_code(code: &Calcit) -> Option<Calcit> {
    let Calcit::List(items) = code else {
      return None;
    };

    match (items.first(), items.get(2)) {
      (Some(Calcit::Symbol { sym, .. }), Some(value)) if sym.as_ref() == "def" => Some(value.to_owned()),
      (Some(Calcit::Import(import)), Some(value)) if import.ns.as_ref() == CORE_NS && import.def.as_ref() == "def" => {
        Some(value.to_owned())
      }
      _ => None,
    }
  }

  fn collect_static_impls(&self) -> Option<Vec<Arc<CalcitImpl>>> {
    match self {
      Self::Struct(struct_def, _) | Self::Record(struct_def) => Some(struct_def.impls.to_vec()),
      Self::Enum(enum_def, _) | Self::Tuple(enum_def) => Some(enum_def.impls().to_vec()),
      Self::Optional(inner) => inner.collect_static_impls(),
      Self::TypeRef(name, _) => resolve_type_ref_as_schema(name).and_then(|schema| schema.collect_static_impls()),
      Self::TypeSlot(name) => resolve_type_slot(name).and_then(|bound| bound.collect_static_impls()),
      _ => self.core_impl_list_symbol().and_then(|symbol| {
        lookup_runtime_ready_registered(CORE_NS, symbol)
          .and_then(|value| Self::collect_impls_from_value(&value))
          .or_else(|| {
            lookup_def_code_registered(CORE_NS, symbol)
              .and_then(|code| Self::extract_def_value_from_code(&code))
              .and_then(|value| Self::collect_impls_from_value(&value))
          })
      }),
    }
  }

  fn infer_builtin_trait_name(imp: &CalcitImpl) -> Option<String> {
    let raw = imp.name().ref_str();
    let body = raw.strip_prefix("&core-")?.strip_suffix("-impl")?;
    let segment = body.split('-').next()?;
    let mut chars = segment.chars();
    let first = chars.next()?;
    let mut name = first.to_uppercase().collect::<String>();
    name.push_str(chars.as_str());
    Some(name)
  }

  fn builtin_type_satisfies_trait(&self, expected_trait: &CalcitTrait) -> bool {
    if expected_trait.name.ref_str() != "Mappable" {
      return false;
    }

    matches!(self, Self::List(_) | Self::Map(_, _) | Self::Set(_) | Self::Optional(_))
  }

  fn impl_matches_trait(imp: &CalcitImpl, expected_trait: &CalcitTrait) -> bool {
    imp.trait_name().is_some_and(|name| name == &expected_trait.name)
      || imp.name() == &expected_trait.name
      || Self::infer_builtin_trait_name(imp).as_deref() == Some(expected_trait.name.ref_str())
  }

  fn satisfies_trait_bound(&self, expected_trait: &CalcitTrait) -> bool {
    if self.builtin_type_satisfies_trait(expected_trait) {
      return true;
    }

    self
      .collect_static_impls()
      .is_some_and(|impls| impls.iter().any(|imp| Self::impl_matches_trait(imp, expected_trait)))
  }

  fn satisfies_trait_bounds(&self, expected_traits: &[Arc<CalcitTrait>]) -> bool {
    expected_traits
      .iter()
      .all(|trait_def| self.satisfies_trait_bound(trait_def.as_ref()))
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
        Self::Unit => true, // nil is always valid for Optional types
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
      | (Self::CirruQuote, Self::CirruQuote)
      | (Self::Unit, Self::Unit) => true,
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
      (Self::TypeRef(a_name, a_args), Self::TypeRef(b_name, b_args)) => {
        if !Self::type_ref_name_matches(a_name, b_name) && !Self::type_ref_name_matches(b_name, a_name) {
          return false;
        }
        if a_args.is_empty() || b_args.is_empty() {
          return true;
        }
        a_args.len() == b_args.len() && a_args.iter().zip(b_args.iter()).all(|(x, y)| x.matches_with_bindings(y, bindings))
      }
      (Self::List(a), Self::List(b)) => a.matches_with_bindings(b, bindings),
      (Self::Map(ak, av), Self::Map(bk, bv)) => ak.matches_with_bindings(bk, bindings) && av.matches_with_bindings(bv, bindings),
      (Self::Set(a), Self::Set(b)) => a.matches_with_bindings(b, bindings),
      (Self::Ref(a), Self::Ref(b)) => a.matches_with_bindings(b, bindings),
      (Self::TypeRef(name, args), Self::Struct(base, other_args)) | (Self::Struct(base, other_args), Self::TypeRef(name, args)) => {
        if !Self::type_ref_name_matches(name, base.name.ref_str()) {
          return false;
        }
        match (args.is_empty(), other_args.is_empty()) {
          (true, true) => true,
          (false, false) => {
            args.len() == other_args.len()
              && args
                .iter()
                .zip(other_args.iter())
                .all(|(x, y)| x.matches_with_bindings(y, bindings))
          }
          (true, false) => Self::bind_declared_generics_from_applied_args(base.generics.as_ref(), other_args.as_ref(), bindings),
          (false, true) => Self::bind_declared_generics_from_applied_args(base.generics.as_ref(), args.as_ref(), bindings),
        }
      }
      (Self::TypeRef(name, args), Self::Enum(base, other_args)) | (Self::Enum(base, other_args), Self::TypeRef(name, args)) => {
        if !Self::type_ref_name_matches(name, base.name().ref_str()) {
          return false;
        }
        if args.is_empty() || other_args.is_empty() {
          return true;
        }
        args.len() == other_args.len()
          && args
            .iter()
            .zip(other_args.iter())
            .all(|(x, y)| x.matches_with_bindings(y, bindings))
      }
      (Self::TypeRef(name, _), Self::Record(base)) | (Self::Record(base), Self::TypeRef(name, _)) => {
        Self::type_ref_name_matches(name, base.name.ref_str())
      }
      (Self::TypeRef(name, _), Self::Tuple(base)) | (Self::Tuple(base), Self::TypeRef(name, _)) => {
        Self::type_ref_name_matches(name, base.name().ref_str())
      }
      (Self::Struct(a, a_args), Self::Struct(b, b_args)) => {
        if a.name != b.name {
          return false;
        }
        match (a_args.is_empty(), b_args.is_empty()) {
          (true, true) => true,
          (false, false) => {
            a_args.len() == b_args.len() && a_args.iter().zip(b_args.iter()).all(|(x, y)| x.matches_with_bindings(y, bindings))
          }
          _ => {
            // one applied, one bare — bind generics from the applied side
            let (base, args) = if !a_args.is_empty() { (a, a_args) } else { (b, b_args) };
            Self::bind_declared_generics_from_applied_args(base.generics.as_ref(), args.as_ref(), bindings)
          }
        }
      }
      (Self::Enum(a, a_args), Self::Enum(b, b_args)) => {
        if a.name() != b.name() {
          return false;
        }
        if a_args.is_empty() || b_args.is_empty() {
          return true;
        }
        a_args.len() == b_args.len() && a_args.iter().zip(b_args.iter()).all(|(x, y)| x.matches_with_bindings(y, bindings))
      }
      (Self::Trait(a), Self::Trait(b)) => a.name == b.name,
      (Self::TraitSet(actual), Self::Trait(expected)) => actual.iter().any(|t| t.name == expected.name),
      (Self::Trait(actual), Self::TraitSet(expected)) => expected.len() == 1 && expected.iter().any(|t| t.name == actual.name),
      (Self::TraitSet(actual), Self::TraitSet(expected)) => expected.iter().all(|t| actual.iter().any(|a| a.name == t.name)),
      (actual, Self::Trait(expected)) => actual.satisfies_trait_bound(expected.as_ref()),
      (actual, Self::TraitSet(expected)) => actual.satisfies_trait_bounds(expected.as_ref()),
      (Self::Struct(_, _), Self::Custom(expected)) if Self::custom_keyword_matches(expected, "struct") => true,
      (Self::Enum(_, _), Self::Custom(expected)) if Self::custom_keyword_matches(expected, "enum") => true,
      (Self::Trait(_), Self::Custom(expected)) if Self::custom_keyword_matches(expected, "trait") => true,
      (Self::TraitSet(_), Self::Custom(expected)) if Self::custom_keyword_matches(expected, "trait") => true,
      (Self::Custom(actual), Self::Custom(expected))
        if Self::custom_keyword_matches(expected, "impl") && matches!(actual.as_ref(), Calcit::Impl(_)) =>
      {
        true
      }
      // DynTuple matches any other DynTuple
      (Self::DynTuple, Self::DynTuple) => true,
      // Function type matching: DynFn matches any Fn, specific Fn must match signature
      (Self::Fn(_), Self::DynFn) | (Self::DynFn, Self::Fn(_)) => true,
      // Tags are callable in Calcit (as map key accessors), so they satisfy :fn requirements
      (Self::Tag, Self::DynFn) | (Self::Tag, Self::Fn(_)) => true,
      (Self::Fn(a), Self::Fn(b)) => a.matches_signature(b.as_ref()),
      (Self::Variadic(a), Self::Variadic(b)) => a.matches_with_bindings(b, bindings),
      (Self::Custom(a), Self::Custom(b)) => a.as_ref() == b.as_ref(),
      (Self::Record(a), Self::Record(b)) => a.name == b.name,
      (Self::Record(a), Self::Struct(b, _)) => a.name == b.name,
      (Self::Record(_), Self::Custom(expected)) if Self::custom_keyword_matches(expected, "record") => true,
      (Self::Tuple(a), Self::Tuple(b)) => a.name() == b.name(),
      (Self::Tuple(a), Self::Enum(b, _)) | (Self::Enum(b, _), Self::Tuple(a)) => a.name() == b.name(),
      (Self::Tuple(_), Self::DynTuple) | (Self::DynTuple, Self::Tuple(_)) => true,
      // TypeRef schema resolution: when a TypeRef doesn't match any concrete type above,
      // try to resolve it as a type alias by looking up the definition's schema.
      (Self::TypeRef(name, _), other) | (other, Self::TypeRef(name, _)) => {
        if let Some(resolved) = resolve_type_ref_as_schema(name) {
          return resolved.matches_with_bindings(other, bindings);
        }
        false
      }
      // TypeSlot: resolve the bound type from the global registry and delegate
      (Self::TypeSlot(name), other) | (other, Self::TypeSlot(name)) => {
        if let Some(resolved) = resolve_type_slot(name) {
          return resolved.matches_with_bindings(other, bindings);
        }
        // Slot not yet bound — treat as Dynamic (no checking)
        true
      }
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
        if tag_name == "dynamic" {
          Self::Dynamic
        } else if matches!(tag_name, "record" | "struct" | "enum" | "trait" | "impl") {
          Self::Custom(Arc::new(Calcit::tag(tag_name)))
        } else if let Some(builtin) = Self::builtin_type_from_tag_name(tag_name) {
          builtin
        } else {
          Self::Tag
        }
      }
      Calcit::List(_) => Self::List(Arc::new(Self::Dynamic)),
      Calcit::Map(_) => Self::Map(Arc::new(Self::Dynamic), Arc::new(Self::Dynamic)),
      Calcit::Set(_) => Self::Set(Arc::new(Self::Dynamic)),
      Calcit::Record(record) => Self::Record(record.struct_ref.clone()),
      Calcit::Enum(enum_def) => Self::Enum(Arc::new(enum_def.to_owned()), Arc::new(vec![])),
      Calcit::Struct(struct_def) => Self::Struct(Arc::new(struct_def.to_owned()), Arc::new(vec![])),
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
        match &tuple.sum_type {
          Some(enum_def) => Self::Tuple(enum_def.clone()),
          None => Self::DynTuple,
        }
      }
      Calcit::Fn { info, .. } => Self::from_function_parts(info.arg_types.clone(), info.return_type.clone()),
      Calcit::Import(import) => Self::from_import(import).unwrap_or(Self::Dynamic),
      Calcit::Proc(proc) => {
        if let Some(signature) = proc.get_type_signature() {
          Self::from_function_parts(signature.arg_types.clone(), signature.return_type.clone())
        } else {
          Self::Dynamic
        }
      }
      Calcit::Ref(_, _) => Self::Ref(Arc::new(Self::Dynamic)),
      Calcit::Symbol { .. } => Self::Symbol,
      Calcit::Buffer(_) => Self::Buffer,
      Calcit::CirruQuote(_) => Self::CirruQuote,
      Calcit::Trait(trait_def) => Self::Trait(Arc::new(trait_def.to_owned())),
      other => Self::Custom(Arc::new(other.to_owned())),
    }
  }

  pub fn from_tag_name(name: &str) -> Self {
    let tag_name = name.trim_start_matches(':');
    if tag_name == "dynamic" {
      Self::Dynamic
    } else {
      Self::builtin_type_from_tag_name(tag_name).unwrap_or_else(|| match tag_name {
        "record" | "struct" | "enum" | "trait" | "impl" => Self::Custom(Arc::new(Calcit::tag(tag_name))),
        _ => Self::Tag,
      })
    }
  }

  pub fn from_function_parts(arg_types: Vec<Arc<CalcitTypeAnnotation>>, return_type: Arc<CalcitTypeAnnotation>) -> Self {
    Self::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types,
      return_type,
      fn_kind: SchemaKind::Fn,
      rest_type: None,
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

    let resolved = lookup_runtime_ready_registered(import.ns.as_ref(), import.def.as_ref())
      .or_else(|| lookup_def_code_registered(import.ns.as_ref(), import.def.as_ref()))
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
      Self::Fn(_) => Calcit::Tag(EdnTag::from("fn")),
      Self::Variadic(inner) => Calcit::Tuple(CalcitTuple {
        tag: Arc::new(Calcit::Tag(EdnTag::from("&"))),
        extra: vec![inner.to_calcit()],
        sum_type: None,
      }),
      Self::Custom(value) => value.as_ref().to_owned(),
      Self::Optional(inner) => Calcit::Tuple(CalcitTuple {
        tag: Arc::new(Calcit::Tag(EdnTag::from("optional"))),
        extra: vec![inner.to_calcit()],
        sum_type: None,
      }),
      Self::Struct(struct_def, args) => {
        if args.is_empty() {
          Calcit::Struct((**struct_def).clone())
        } else {
          let mut items = Vec::with_capacity(args.len() + 2);
          items.push(Self::make_symbol("::"));
          let base_name = struct_def.name.ref_str().trim_start_matches(':');
          items.push(Self::make_symbol(base_name));
          for arg in args.iter() {
            items.push(arg.to_calcit());
          }
          Calcit::List(Arc::new(CalcitList::from(items.as_slice())))
        }
      }
      Self::TypeVar(name) => Self::quote_symbol(name),
      Self::TypeRef(name, args) => {
        if args.is_empty() {
          Self::quote_symbol(name)
        } else {
          let mut items = Vec::with_capacity(args.len() + 2);
          items.push(Self::make_symbol("::"));
          items.push(Self::quote_symbol(name));
          for arg in args.iter() {
            items.push(arg.to_calcit());
          }
          Calcit::List(Arc::new(CalcitList::from(items.as_slice())))
        }
      }
      Self::Enum(enum_def, _) => Calcit::Enum((**enum_def).clone()),
      Self::Trait(trait_def) => Calcit::Trait((**trait_def).clone()),
      Self::TraitSet(_) => Calcit::Nil,
      Self::Dynamic => Calcit::Tag(EdnTag::from("dynamic")),
      _ => Calcit::Nil,
    }
  }

  /// Convert this type annotation to its [`Edn`] representation for schema serialization.
  /// This is the inverse of `parse_fn_schema_from_edn` + `edn_type_to_calcit` + `parse_type_annotation_form`.
  pub fn to_type_edn(&self) -> Edn {
    match self {
      // Simple builtin scalars
      Self::Dynamic => Edn::tag("dynamic"),
      Self::Unit => Edn::tag("unit"),
      Self::Bool => Edn::tag("bool"),
      Self::Number => Edn::tag("number"),
      Self::String => Edn::tag("string"),
      Self::Symbol => Edn::tag("symbol"),
      Self::Tag => Edn::tag("tag"),
      Self::DynFn => Edn::tag("fn"),
      Self::DynTuple => Edn::tag("tuple"),
      Self::Buffer => Edn::tag("buffer"),
      Self::CirruQuote => Edn::tag("cirru-quote"),
      // TypeVar: source syntax uses `'T`, while Cirru EDN stores that as `Edn::Symbol("T")`.
      Self::TypeVar(name) => Edn::Symbol(Arc::from(name.trim_start_matches('\''))),
      Self::TypeRef(name, args) => {
        if args.is_empty() {
          Edn::Symbol(Arc::from(name.trim_start_matches('\'')))
        } else {
          Edn::tuple(
            Edn::Symbol(Arc::from(name.trim_start_matches('\''))),
            args.iter().map(|arg| arg.to_type_edn()).collect(),
          )
        }
      }
      // Parameterized builtins – keep inner type if non-dynamic
      Self::List(inner) => {
        if matches!(inner.as_ref(), Self::Dynamic) {
          Edn::tag("list")
        } else {
          Edn::tuple(Edn::tag("list"), vec![inner.to_type_edn()])
        }
      }
      Self::Map(k, v) => {
        if matches!(k.as_ref(), Self::Dynamic) && matches!(v.as_ref(), Self::Dynamic) {
          Edn::tag("map")
        } else {
          Edn::tuple(Edn::tag("map"), vec![k.to_type_edn(), v.to_type_edn()])
        }
      }
      Self::Set(inner) => {
        if matches!(inner.as_ref(), Self::Dynamic) {
          Edn::tag("set")
        } else {
          Edn::tuple(Edn::tag("set"), vec![inner.to_type_edn()])
        }
      }
      Self::Ref(inner) => {
        if matches!(inner.as_ref(), Self::Dynamic) {
          Edn::tag("ref")
        } else {
          Edn::tuple(Edn::tag("ref"), vec![inner.to_type_edn()])
        }
      }
      Self::Optional(inner) => Edn::tuple(Edn::tag("optional"), vec![inner.to_type_edn()]),
      Self::Variadic(inner) => Edn::tuple(Edn::tag("&"), vec![inner.to_type_edn()]),
      Self::Fn(fn_annot) => Edn::tuple(Edn::tag("fn"), vec![fn_annot.to_inline_type_schema_edn()]),
      Self::Struct(s, args) => {
        if args.is_empty() {
          Edn::Tag(s.name.clone())
        } else {
          let mut items = vec![Edn::Symbol(Arc::from("::"))];
          let base_name = s.name.ref_str().trim_start_matches(':');
          items.push(Edn::Symbol(Arc::from(base_name)));
          for arg in args.iter() {
            items.push(arg.to_type_edn());
          }
          Edn::List(EdnListView(items))
        }
      }
      // Custom Calcit values – do a best-effort Calcit→Edn conversion
      Self::Custom(value) => calcit_type_to_edn(value.as_ref()),
      // Enum / Trait variants – use the name as a symbol
      Self::Enum(e, _) => Edn::Tag(e.name().clone()),
      Self::Record(struct_def) => Edn::Tag(struct_def.name.clone()),
      Self::Tuple(enum_def) => Edn::Tag(enum_def.name().clone()),
      Self::Trait(trait_def) => Edn::Tag(trait_def.name.clone()),
      Self::TypeSlot(name) => Edn::Symbol(Arc::from(format!("*{name}"))),
      // Anything else falls back to dynamic
      _ => Edn::tag("dynamic"),
    }
  }

  pub fn as_record(&self) -> Option<&CalcitStruct> {
    self.as_struct()
  }

  pub fn as_tuple(&self) -> Option<&CalcitEnum> {
    match self {
      Self::Tuple(enum_def) => Some(enum_def),
      Self::Enum(enum_def, _) => Some(enum_def),
      Self::Optional(inner) => inner.as_tuple(),
      _ => None,
    }
  }

  pub fn as_struct(&self) -> Option<&CalcitStruct> {
    match self {
      Self::Record(struct_def) => Some(struct_def),
      Self::Struct(struct_def, _) => Some(struct_def),
      Self::Custom(value) => match value.as_ref() {
        Calcit::Record(record) => Some(record.struct_ref.as_ref()),
        Calcit::Struct(struct_def) => Some(struct_def),
        _ => None,
      },
      Self::Optional(inner) => inner.as_struct(),
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
      Self::Fn(signature) => signature.describe(),
      Self::Variadic(inner) => format!("variadic {}", inner.describe()),
      Self::Custom(_) => "custom".to_string(),
      Self::Optional(inner) => format!("optional<{}>", inner.describe()),
      Self::Struct(base, args) => {
        if args.is_empty() {
          format!("struct {}", base.name)
        } else {
          let rendered = args.iter().map(|t| t.describe()).collect::<Vec<_>>().join(", ");
          format!("struct {}<{}>", base.name, rendered)
        }
      }
      Self::TypeVar(name) => format!("'{name}"),
      Self::TypeRef(name, args) => {
        if args.is_empty() {
          format!("type {name}")
        } else {
          let rendered = args.iter().map(|t| t.describe()).collect::<Vec<_>>().join(", ");
          format!("type {name}<{rendered}>")
        }
      }
      Self::Enum(enum_def, args) => {
        if args.is_empty() {
          format!("enum {}", enum_def.name())
        } else {
          let rendered = args.iter().map(|t| t.describe()).collect::<Vec<_>>().join(", ");
          format!("enum {}<{}>", enum_def.name(), rendered)
        }
      }
      Self::Record(struct_def) => format!("struct {}", struct_def.name),
      Self::Tuple(enum_def) => format!("enum {}", enum_def.name()),
      Self::Dynamic => "dynamic".to_string(),
      Self::TypeSlot(name) => format!("type-slot({name})"),
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
      Self::TypeRef(_, _) => 22,
      Self::Struct(_, _) => 23,
      Self::Enum(_, _) => 24,
      Self::Trait(_) => 25,
      Self::TraitSet(_) => 26,
      Self::Unit => 27,
      Self::TypeSlot(_) => 28,
    }
  }
}

fn resolve_struct_annotation(struct_form: &Calcit, class_form: Option<&Calcit>) -> Option<CalcitStruct> {
  let mut struct_def = resolve_struct_def(struct_form)?;
  if let Some(class_record) = class_form.and_then(resolve_record_def) {
    struct_def.impls = vec![Arc::new(CalcitImpl::from_record(&class_record))];
  }
  Some(struct_def)
}

fn resolve_enum_annotation(enum_form: &Calcit, class_form: Option<&Calcit>) -> Option<CalcitEnum> {
  let mut enum_def = resolve_enum_def(enum_form)?;
  if let Some(class_record) = class_form.and_then(resolve_record_def) {
    enum_def.set_impls(vec![Arc::new(CalcitImpl::from_record(&class_record))]);
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

/// Resolve a struct definition by namespace and definition name from the program registry.
/// Used by `CalcitTypeAnnotation::resolve_to_struct` to look up `TypeRef("ns/def")` at compile time.
fn resolve_struct_from_program(ns: &str, def: &str) -> Option<CalcitStruct> {
  lookup_runtime_ready_registered(ns, def)
    .and_then(|value| match &value {
      Calcit::Struct(s) => Some(s.to_owned()),
      _ => resolve_type_def_from_code(&value).and_then(|resolved| match resolved {
        Calcit::Struct(s) => Some(s),
        _ => None,
      }),
    })
    .or_else(|| {
      lookup_def_code_registered(ns, def).and_then(|code| {
        resolve_type_def_from_code(&code).and_then(|resolved| match resolved {
          Calcit::Struct(s) => Some(s),
          _ => None,
        })
      })
    })
}

/// Resolve an enum definition by namespace and definition name from the program registry.
/// Used by `CalcitTypeAnnotation::resolve_to_enum` to look up `TypeRef("ns/def")` at compile time.
fn resolve_enum_from_program(ns: &str, def: &str) -> Option<CalcitEnum> {
  lookup_runtime_ready_registered(ns, def)
    .and_then(|value| match &value {
      Calcit::Enum(e) => Some(e.to_owned()),
      Calcit::Record(record) => CalcitEnum::from_record(record.to_owned()).ok(),
      _ => resolve_type_def_from_code(&value).and_then(|resolved| match resolved {
        Calcit::Enum(e) => Some(e),
        Calcit::Record(record) => CalcitEnum::from_record(record).ok(),
        _ => None,
      }),
    })
    .or_else(|| {
      lookup_def_code_registered(ns, def).and_then(|code| {
        resolve_type_def_from_code(&code).and_then(|resolved| match resolved {
          Calcit::Enum(e) => Some(e),
          Calcit::Record(record) => CalcitEnum::from_record(record).ok(),
          _ => None,
        })
      })
    })
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

/// Convert a simple [`Calcit`] type-form back to its [`Edn`] representation.
/// Used as a fallback inside [`CalcitTypeAnnotation::to_type_edn`] for `Custom` variants.
fn calcit_type_to_edn(form: &Calcit) -> Edn {
  match form {
    Calcit::Nil => Edn::Nil,
    Calcit::Tag(t) => Edn::Tag(t.clone()),
    Calcit::Symbol { sym, .. } => Edn::Symbol(sym.clone()),
    Calcit::List(xs) => Edn::List(EdnListView(xs.iter().map(calcit_type_to_edn).collect())),
    Calcit::Tuple(t) => Edn::tuple(calcit_type_to_edn(t.tag.as_ref()), t.extra.iter().map(calcit_type_to_edn).collect()),
    _ => Edn::Nil,
  }
}

fn resolve_type_def_from_code(code: &Calcit) -> Option<Calcit> {
  // Unwrap thunks: defstruct/defenum definitions are stored as unevaluated thunks
  if let Calcit::Thunk(thunk) = code {
    return resolve_type_def_from_code(thunk.get_code());
  }
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
    let field_type = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(field_type_form, generics.as_slice());
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
  let mut generics: Vec<Arc<str>> = vec![];
  let mut start_idx = 2;

  if let Some(generics_form) = items.get(2) {
    if let Some(vars) = CalcitTypeAnnotation::parse_generics_list(generics_form) {
      generics = vars;
      start_idx = 3;
    }
  }

  let mut variants: Vec<(EdnTag, Calcit)> = Vec::new();
  for item in items.iter().skip(start_idx) {
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
  generics.sort();
  generics.dedup();
  let mut struct_ref = CalcitStruct::from_fields(name, fields);
  struct_ref.generics = Arc::new(generics);
  let record = CalcitRecord {
    struct_ref: Arc::new(struct_ref),
    values: Arc::new(values),
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

      let resolved = lookup_runtime_ready_registered(import.ns.as_ref(), import.def.as_ref())
        .map(|value| resolve_type_def_from_code(&value).unwrap_or(value))
        .or_else(|| {
          lookup_def_code_registered(import.ns.as_ref(), import.def.as_ref())
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
    Calcit::Symbol { sym, info, .. } => lookup_runtime_ready_registered(info.at_ns.as_ref(), sym)
      .map(|value| resolve_type_def_from_code(&value).unwrap_or(value))
      .or_else(|| {
        lookup_def_code_registered(info.at_ns.as_ref(), sym).map(|value| resolve_type_def_from_code(&value).unwrap_or(value))
      }),
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
    let arg_types = CalcitTypeAnnotation::collect_arg_type_hints_from_body(&body_items, &params, &[]);

    assert!(matches!(arg_types[0].as_ref(), CalcitTypeAnnotation::Number));
  }

  #[test]
  fn extracts_return_type_from_schema_first_hint() {
    let ns: Arc<str> = Arc::from("tests");
    let hint_form = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::HintFn, ns.clone()),
      Calcit::List(Arc::new(CalcitList::from(&[
        symbol("{}"),
        Calcit::List(Arc::new(CalcitList::from(&[
          Calcit::Tag(EdnTag::from("return")),
          Calcit::Tag(EdnTag::from("number")),
        ]))),
      ]))),
    ])));

    let detected = CalcitTypeAnnotation::extract_return_type_from_hint_form(&hint_form).expect("return type from schema");
    assert!(matches!(detected.as_ref(), CalcitTypeAnnotation::Number));
  }

  #[test]
  fn extracts_generics_from_schema_first_hint() {
    let ns: Arc<str> = Arc::from("tests");
    let hint_form = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::HintFn, ns.clone()),
      Calcit::List(Arc::new(CalcitList::from(&[
        symbol("{}"),
        Calcit::List(Arc::new(CalcitList::from(&[
          Calcit::Tag(EdnTag::from("generics")),
          Calcit::List(Arc::new(CalcitList::from(&[symbol("T"), symbol("U")]))),
        ]))),
      ]))),
    ])));

    let vars = CalcitTypeAnnotation::extract_generics_from_hint_form(&hint_form).expect("generics from schema");
    assert_eq!(vars, vec![Arc::from("T"), Arc::from("U")]);
  }

  #[test]
  fn scoped_parser_distinguishes_type_var_and_named_ref() {
    let generics = vec![Arc::from("T")];

    let generic = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(&symbol("T"), generics.as_slice());
    assert!(matches!(generic.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T"));

    let quoted_result = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, Arc::from(CORE_NS)),
      symbol("Result"),
    ])));
    let named = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(&quoted_result, generics.as_slice());
    assert!(matches!(named.as_ref(), CalcitTypeAnnotation::TypeRef(name, args) if name.as_ref() == "Result" && args.is_empty()));
  }

  #[test]
  fn scoped_parser_keeps_named_type_applications() {
    let generics = vec![Arc::from("T"), Arc::from("E")];
    let applied_named_items = vec![
      symbol("::"),
      Calcit::List(Arc::new(CalcitList::from(&[
        Calcit::Syntax(CalcitSyntax::Quote, Arc::from(CORE_NS)),
        symbol("Result"),
      ]))),
      symbol("T"),
      symbol("E"),
    ];
    let applied_named = Calcit::List(Arc::new(CalcitList::from(applied_named_items.as_slice())));

    let parsed = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(&applied_named, generics.as_slice());
    assert!(matches!(parsed.as_ref(), CalcitTypeAnnotation::TypeRef(name, args) if name.as_ref() == "Result" && args.len() == 2));
    let CalcitTypeAnnotation::TypeRef(_, args) = parsed.as_ref() else {
      panic!("expected named type application, got {parsed:?}");
    };
    assert!(matches!(args.first().map(|t| t.as_ref()), Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "T"));
    assert!(matches!(args.get(1).map(|t| t.as_ref()), Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "E"));
  }

  #[test]
  fn parses_hashmap_fn_type_syntax() {
    let type_var_t = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, Arc::from(CORE_NS)),
      symbol("T"),
    ])));
    let payload_items = vec![
      symbol("{}"),
      Calcit::List(Arc::new(CalcitList::from(&[
        Calcit::Tag(EdnTag::from("generics")),
        Calcit::List(Arc::new(CalcitList::from(&[symbol("[]"), symbol("T")]))),
      ]))),
      Calcit::List(Arc::new(CalcitList::from(&[
        Calcit::Tag(EdnTag::from("args")),
        Calcit::List(Arc::new(CalcitList::from(&[symbol("[]"), type_var_t.to_owned()]))),
      ]))),
      Calcit::List(Arc::new(CalcitList::from(&[Calcit::Tag(EdnTag::from("return")), type_var_t]))),
    ];
    let payload = Calcit::List(Arc::new(CalcitList::from(payload_items.as_slice())));
    let form = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("::"),
      Calcit::Tag(EdnTag::from("fn")),
      payload,
    ])));

    let parsed = CalcitTypeAnnotation::parse_type_annotation_form(&form);
    assert!(matches!(parsed.as_ref(), CalcitTypeAnnotation::Fn(fn_annot) if fn_annot.generics.as_ref() == &[Arc::from("T")]));
    let CalcitTypeAnnotation::Fn(fn_annot) = parsed.as_ref() else {
      panic!("expected fn annotation, got {parsed:?}");
    };
    assert!(
      matches!(fn_annot.arg_types.first().map(|t| t.as_ref()), Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "T")
    );
    assert!(matches!(fn_annot.return_type.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T"));
  }

  #[test]
  fn fn_annotation_serializes_to_hashmap_payload() {
    let annotation = CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))],
      return_type: Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T"))),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
    }));

    let edn = annotation.to_type_edn();
    let Edn::Tuple(view) = &edn else {
      panic!("fn annotation should serialize as tuple, got {edn:?}");
    };
    assert!(matches!(view.tag.as_ref(), Edn::Tag(tag) if tag.ref_str() == "fn"));
    let Some(Edn::Map(map)) = view.extra.first() else {
      panic!("fn payload should be schema map: {edn:?}");
    };
    assert!(
      map.tag_get("kind").is_none(),
      "nested fn payload should omit default :kind :fn: {edn:?}"
    );
  }

  #[test]
  fn legacy_positional_fn_syntax_falls_back_to_dynamic_fn() {
    let type_var_a = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, Arc::from(CORE_NS)),
      symbol("A"),
    ])));
    let type_var_b = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, Arc::from(CORE_NS)),
      symbol("B"),
    ])));
    let form = Calcit::List(Arc::new(CalcitList::from(
      vec![
        symbol("::"),
        Calcit::Tag(EdnTag::from("fn")),
        Calcit::List(Arc::new(CalcitList::from(vec![symbol("[]"), type_var_a.clone()].as_slice()))),
        type_var_b.clone(),
      ]
      .as_slice(),
    )));

    let parsed = CalcitTypeAnnotation::parse_type_annotation_form(&form);
    assert!(matches!(parsed.as_ref(), CalcitTypeAnnotation::DynFn));
  }

  #[test]
  fn malformed_nested_fn_schema_falls_back_to_dynfn_on_return_only_payload() {
    let payload = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("{}"),
      Calcit::List(Arc::new(CalcitList::from(&[Calcit::Nil, Calcit::Tag(EdnTag::from("bool"))]))),
    ])));
    let form = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("::"),
      Calcit::Tag(EdnTag::from("fn")),
      payload,
    ])));

    let parsed = CalcitTypeAnnotation::parse_type_annotation_form(&form);
    assert!(matches!(parsed.as_ref(), CalcitTypeAnnotation::DynFn));
  }

  #[test]
  fn malformed_nested_fn_schema_falls_back_to_dynfn_on_args_only_payload() {
    let payload = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("{}"),
      Calcit::List(Arc::new(CalcitList::from(&[
        Calcit::Nil,
        Calcit::List(Arc::new(CalcitList::from(&[symbol("[]"), Calcit::Tag(EdnTag::from("number"))]))),
      ]))),
    ])));
    let form = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("::"),
      Calcit::Tag(EdnTag::from("fn")),
      payload,
    ])));

    let parsed = CalcitTypeAnnotation::parse_type_annotation_form(&form);
    assert!(matches!(parsed.as_ref(), CalcitTypeAnnotation::DynFn));
  }

  #[test]
  fn malformed_empty_nested_fn_schema_becomes_dynfn() {
    let payload = Calcit::List(Arc::new(CalcitList::from(vec![symbol("{}")].as_slice())));
    let form = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("::"),
      Calcit::Tag(EdnTag::from("fn")),
      payload,
    ])));

    let parsed = CalcitTypeAnnotation::parse_type_annotation_form(&form);
    assert!(matches!(parsed.as_ref(), CalcitTypeAnnotation::DynFn));
  }

  #[test]
  fn nested_macro_fn_annotation_keeps_kind() {
    let annotation = CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![],
      return_type: Arc::new(CalcitTypeAnnotation::Bool),
      fn_kind: SchemaKind::Macro,
      rest_type: None,
    }));

    let edn = annotation.to_type_edn();
    let Edn::Tuple(view) = &edn else {
      panic!("fn annotation should serialize as tuple, got {edn:?}");
    };
    let Some(Edn::Map(map)) = view.extra.first() else {
      panic!("fn payload should be schema map: {edn:?}");
    };
    assert!(matches!(map.tag_get("kind"), Some(Edn::Tag(tag)) if tag.ref_str() == "macro"));
    assert!(matches!(map.tag_get("return"), Some(Edn::Tag(tag)) if tag.ref_str() == "bool"));
  }

  #[test]
  fn parse_fn_schema_keeps_explicit_dynamic_schema() {
    let schema = Edn::Map(EdnMapView::from(HashMap::from([
      (Edn::tag("kind"), Edn::tag("fn")),
      (Edn::tag("args"), Edn::List(EdnListView(vec![]))),
      (Edn::tag("return"), Edn::tag("dynamic")),
    ])));

    let parsed = CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema).expect("explicit fn schema should be preserved");
    assert_eq!(parsed.fn_kind, SchemaKind::Fn);
    assert!(parsed.arg_types.is_empty());
    assert!(matches!(parsed.return_type.as_ref(), CalcitTypeAnnotation::Dynamic));
  }

  #[test]
  fn parse_wrapped_top_level_fn_schema_from_edn() {
    let schema = Edn::tuple(
      Edn::tag("fn"),
      vec![Edn::Map(EdnMapView::from(HashMap::from([
        (Edn::tag("args"), Edn::List(EdnListView(vec![Edn::tag("number")]))),
        (Edn::tag("return"), Edn::tag("string")),
      ])))],
    );

    let parsed = CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema).expect("wrapped schema should parse");
    assert!(matches!(parsed.arg_types.as_slice(), [arg] if matches!(arg.as_ref(), CalcitTypeAnnotation::Number)));
    assert!(matches!(parsed.return_type.as_ref(), CalcitTypeAnnotation::String));
  }

  #[test]
  fn parse_wrapped_top_level_fn_schema_from_edn_keeps_where_bounds() {
    let schema = Edn::tuple(
      Edn::tag("fn"),
      vec![Edn::Map(EdnMapView::from(HashMap::from([
        (Edn::tag("args"), Edn::List(EdnListView(vec![Edn::Symbol(Arc::from("C"))]))),
        (Edn::tag("generics"), Edn::List(EdnListView(vec![Edn::Symbol(Arc::from("C"))]))),
        (
          Edn::tag("where"),
          Edn::Map(EdnMapView::from(HashMap::from([(
            Edn::Symbol(Arc::from("C")),
            Edn::Symbol(Arc::from("Mappable")),
          )]))),
        ),
        (Edn::tag("return"), Edn::tag("dynamic")),
      ])))],
    );

    let parsed = CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema).expect("wrapped schema with where should parse");
    assert_eq!(parsed.where_bounds.len(), 1);
    assert_eq!(parsed.where_bounds[0].name.as_ref(), "C");
    assert_eq!(parsed.where_bounds[0].traits.len(), 1);
    assert_eq!(parsed.where_bounds[0].traits[0].name.ref_str(), "Mappable");
  }

  #[test]
  fn extracts_where_bounds_from_hint_form() {
    let show_trait = CalcitTrait::new(
      EdnTag::new("Show"),
      vec![EdnTag::new("show")],
      vec![crate::calcit::DYNAMIC_TYPE.clone()],
    );
    let generics = Calcit::List(Arc::new(CalcitList::from(&[symbol("[]"), symbol("T")])));
    let where_map = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("{}"),
      Calcit::List(Arc::new(CalcitList::from(&[symbol("T"), Calcit::Trait(show_trait.clone())]))),
    ])));
    let schema_map = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("{}"),
      Calcit::List(Arc::new(CalcitList::from(&[Calcit::tag("generics"), generics]))),
      Calcit::List(Arc::new(CalcitList::from(&[Calcit::tag("where"), where_map]))),
    ])));
    let hint_form = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::HintFn, Arc::from("tests.where")),
      schema_map,
    ])));

    let bounds = CalcitTypeAnnotation::extract_where_bounds_from_hint_form(&hint_form).expect("where bounds should parse");
    assert_eq!(bounds.len(), 1);
    assert_eq!(bounds[0].name.as_ref(), "T");
    assert_eq!(bounds[0].traits.len(), 1);
    assert_eq!(bounds[0].traits[0].name.ref_str(), "Show");
  }

  #[test]
  fn wrapped_top_level_fn_schema_emits_where_bounds_in_edn_shape() {
    let schema = CalcitFnTypeAnnotation {
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![CalcitGenericBound {
        name: Arc::from("T"),
        traits: Arc::new(vec![Arc::new(CalcitTrait::new(
          EdnTag::new("Show"),
          vec![EdnTag::new("show")],
          vec![crate::calcit::DYNAMIC_TYPE.clone()],
        ))]),
      }]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))],
      return_type: Arc::new(CalcitTypeAnnotation::String),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
    };

    let edn = schema.to_wrapped_schema_edn();
    let Edn::Tuple(view) = edn else {
      panic!("wrapped schema should serialize as tuple");
    };
    let Some(Edn::Map(map)) = view.extra.first() else {
      panic!("wrapped schema payload should be a map");
    };
    let Some(Edn::Map(where_map)) = map.tag_get("where") else {
      panic!("wrapped schema should contain where map");
    };
    assert!(matches!(where_map.0.get(&Edn::Symbol(Arc::from("T"))), Some(Edn::Tag(tag)) if tag.ref_str() == "Show"));
  }

  #[test]
  fn wrapped_top_level_fn_schema_emits_multi_trait_where_bounds() {
    let schema = CalcitFnTypeAnnotation {
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![CalcitGenericBound {
        name: Arc::from("T"),
        traits: Arc::new(vec![
          Arc::new(CalcitTrait::new(
            EdnTag::new("Show"),
            vec![EdnTag::new("show")],
            vec![crate::calcit::DYNAMIC_TYPE.clone()],
          )),
          Arc::new(CalcitTrait::new(
            EdnTag::new("Eq"),
            vec![EdnTag::new("eq")],
            vec![crate::calcit::DYNAMIC_TYPE.clone()],
          )),
        ]),
      }]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))],
      return_type: Arc::new(CalcitTypeAnnotation::String),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
    };

    let edn = schema.to_wrapped_schema_edn();
    let Edn::Tuple(view) = edn else {
      panic!("wrapped schema should serialize as tuple");
    };
    let Some(Edn::Map(map)) = view.extra.first() else {
      panic!("wrapped schema payload should be a map");
    };
    let Some(Edn::Map(where_map)) = map.tag_get("where") else {
      panic!("wrapped schema should contain where map");
    };
    let Some(Edn::List(traits)) = where_map.0.get(&Edn::Symbol(Arc::from("T"))) else {
      panic!("multi trait where-bound should serialize as list");
    };
    assert_eq!(traits.0.len(), 2);
    assert!(matches!(traits.0.first(), Some(Edn::Tag(tag)) if tag.ref_str() == "Show"));
    assert!(matches!(traits.0.get(1), Some(Edn::Tag(tag)) if tag.ref_str() == "Eq"));
  }

  #[test]
  fn parse_fn_schema_rejects_legacy_quoted_generic_symbol() {
    let schema = Edn::Map(EdnMapView::from(HashMap::from([
      (Edn::tag("kind"), Edn::tag("fn")),
      (Edn::tag("args"), Edn::List(EdnListView(vec![Edn::tag("number")]))),
      (Edn::tag("generics"), Edn::List(EdnListView(vec![Edn::Symbol(Arc::from("'T"))]))),
      (Edn::tag("return"), Edn::tag("number")),
    ])));

    assert!(
      CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema).is_none(),
      "legacy quoted generic symbol should be rejected"
    );
  }

  #[test]
  fn parse_wrapped_top_level_macro_schema_from_edn() {
    let schema = Edn::tuple(
      Edn::tag("macro"),
      vec![Edn::Map(EdnMapView::from(HashMap::from([
        (Edn::tag("args"), Edn::List(EdnListView(vec![Edn::tag("dynamic")]))),
        (Edn::tag("return"), Edn::tag("dynamic")),
      ])))],
    );

    let parsed = CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema).expect("wrapped macro schema should parse");
    assert_eq!(parsed.fn_kind, SchemaKind::Macro);
    assert!(matches!(parsed.arg_types.as_slice(), [arg] if matches!(arg.as_ref(), CalcitTypeAnnotation::Dynamic)));
  }

  #[test]
  fn wrapped_top_level_fn_schema_omits_default_kind_but_keeps_rest() {
    let schema = CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::Number)],
      return_type: Arc::new(CalcitTypeAnnotation::String),
      fn_kind: SchemaKind::Fn,
      rest_type: Some(Arc::new(CalcitTypeAnnotation::Tag)),
    };

    let edn = schema.to_wrapped_schema_edn();
    let Edn::Tuple(view) = edn else {
      panic!("wrapped schema should serialize as tuple");
    };
    let Some(Edn::Map(map)) = view.extra.first() else {
      panic!("wrapped schema payload should be a map");
    };
    assert!(map.tag_get("kind").is_none(), "default fn kind should be omitted");
    assert!(matches!(map.tag_get("rest"), Some(Edn::Tag(tag)) if tag.ref_str() == "tag"));
  }

  #[test]
  fn wrapped_top_level_macro_schema_uses_macro_tag_and_omits_inner_kind() {
    let schema = CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::Dynamic)],
      return_type: Arc::new(CalcitTypeAnnotation::Dynamic),
      fn_kind: SchemaKind::Macro,
      rest_type: Some(Arc::new(CalcitTypeAnnotation::Dynamic)),
    };

    let edn = schema.to_wrapped_schema_edn();
    let Edn::Tuple(view) = edn else {
      panic!("wrapped schema should serialize as tuple");
    };
    assert!(matches!(view.tag.as_ref(), Edn::Tag(tag) if tag.ref_str() == "macro"));
    let Some(Edn::Map(map)) = view.extra.first() else {
      panic!("wrapped schema payload should be a map");
    };
    assert!(
      map.tag_get("kind").is_none(),
      "wrapped macro schema should omit redundant inner :kind"
    );
    assert!(map.tag_get("return").is_none(), "wrapped macro schema should omit return field");
    assert!(matches!(map.tag_get("rest"), Some(Edn::Tag(tag)) if tag.ref_str() == "dynamic"));
  }

  #[test]
  fn wrapped_top_level_macro_schema_keeps_non_dynamic_return() {
    let schema = CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::Dynamic)],
      return_type: Arc::new(CalcitTypeAnnotation::Custom(Arc::new(Calcit::tag("record")))),
      fn_kind: SchemaKind::Macro,
      rest_type: None,
    };

    let edn = schema.to_wrapped_schema_edn();
    let Edn::Tuple(view) = edn else {
      panic!("wrapped schema should serialize as tuple");
    };
    let Some(Edn::Map(map)) = view.extra.first() else {
      panic!("wrapped schema payload should be a map");
    };
    assert!(matches!(map.tag_get("return"), Some(Edn::Tag(tag)) if tag.ref_str() == "record"));
  }

  #[test]
  fn any_is_not_dynamic_alias() {
    assert!(matches!(
      CalcitTypeAnnotation::from_tag_name("dynamic"),
      CalcitTypeAnnotation::Dynamic
    ));
    assert!(!matches!(CalcitTypeAnnotation::from_tag_name("any"), CalcitTypeAnnotation::Dynamic));
    assert!(!matches!(
      CalcitTypeAnnotation::from_calcit(&Calcit::Tag(EdnTag::from("any"))),
      CalcitTypeAnnotation::Dynamic
    ));
  }

  #[test]
  fn rejects_type_args_on_non_generic_struct_annotation() {
    let pair = CalcitStruct {
      name: EdnTag::new("Pair"),
      fields: Arc::new(vec![]),
      field_types: Arc::new(vec![]),
      generics: Arc::new(vec![]),
      impls: vec![],
    };
    let annotation = CalcitTypeAnnotation::Struct(
      Arc::new(pair),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)]),
    );

    let err = annotation
      .validate_applied_type_args()
      .expect_err("non-generic struct should reject type args");
    assert!(err.contains("struct `Pair` is not generic"), "unexpected error: {err}");
  }

  #[test]
  fn rejects_wrong_arity_on_generic_struct_annotation() {
    let pair = CalcitStruct {
      name: EdnTag::new("Pair"),
      fields: Arc::new(vec![]),
      field_types: Arc::new(vec![]),
      generics: Arc::new(vec![Arc::from("A"), Arc::from("B")]),
      impls: vec![],
    };
    let annotation = CalcitTypeAnnotation::Struct(Arc::new(pair), Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]));

    let err = annotation
      .validate_applied_type_args()
      .expect_err("generic struct should enforce arity");
    assert!(
      err.contains("expects 2 type argument(s), but received 1"),
      "unexpected error: {err}"
    );
  }

  #[test]
  fn matching_named_struct_ref_binds_generic_args_from_struct_annotation() {
    let pair = Arc::new(CalcitStruct {
      name: EdnTag::new("Pair"),
      fields: Arc::new(vec![]),
      field_types: Arc::new(vec![]),
      generics: Arc::new(vec![Arc::from("A"), Arc::from("B")]),
      impls: vec![],
    });
    let actual = CalcitTypeAnnotation::Struct(
      pair.clone(),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)]),
    );
    let expected = CalcitTypeAnnotation::TypeRef(Arc::from("Pair"), Arc::new(vec![]));
    let mut bindings = TypeBindings::new();

    assert!(actual.matches_with_bindings(&expected, &mut bindings));
    assert!(matches!(bindings.get("A"), Some(bound) if matches!(bound.as_ref(), CalcitTypeAnnotation::Number)));
    assert!(matches!(bindings.get("B"), Some(bound) if matches!(bound.as_ref(), CalcitTypeAnnotation::String)));
  }

  #[test]
  fn matching_bare_struct_annotation_binds_generic_args_from_named_struct_ref() {
    let pair = Arc::new(CalcitStruct {
      name: EdnTag::new("Pair"),
      fields: Arc::new(vec![]),
      field_types: Arc::new(vec![]),
      generics: Arc::new(vec![Arc::from("A"), Arc::from("B")]),
      impls: vec![],
    });
    let actual = CalcitTypeAnnotation::TypeRef(
      Arc::from("Pair"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)]),
    );
    let expected = CalcitTypeAnnotation::Struct(pair, Arc::new(vec![]));
    let mut bindings = TypeBindings::new();

    assert!(actual.matches_with_bindings(&expected, &mut bindings));
    assert!(matches!(bindings.get("A"), Some(bound) if matches!(bound.as_ref(), CalcitTypeAnnotation::Number)));
    assert!(matches!(bindings.get("B"), Some(bound) if matches!(bound.as_ref(), CalcitTypeAnnotation::String)));
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
      Self::Record(struct_def) => {
        "record".hash(state);
        struct_def.name.hash(state);
        struct_def.fields.hash(state);
      }
      Self::Tuple(enum_def) => {
        "tuple".hash(state);
        enum_def.name().hash(state);
      }
      Self::DynTuple => "dyntuple".hash(state),
      Self::DynFn => "dynfn".hash(state),
      Self::Fn(signature) => {
        "function".hash(state);
        signature.generics.hash(state);
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
      Self::Struct(struct_def, args) => {
        "struct".hash(state);
        struct_def.name.hash(state);
        struct_def.fields.hash(state);
        struct_def.field_types.hash(state);
        struct_def.generics.hash(state);
        args.hash(state);
      }
      Self::TypeVar(name) => {
        "typevar".hash(state);
        name.hash(state);
      }
      Self::TypeRef(name, args) => {
        "typeref".hash(state);
        name.hash(state);
        args.hash(state);
      }
      Self::Enum(enum_def, args) => {
        "enum".hash(state);
        enum_def.name().hash(state);
        args.hash(state);
      }
      Self::Trait(trait_def) => {
        "trait".hash(state);
        trait_def.name.hash(state);
      }
      Self::TraitSet(traits) => {
        "traits".hash(state);
        for t in traits.iter() {
          t.name.hash(state);
        }
      }
      Self::Unit => "unit".hash(state),
      Self::TypeSlot(name) => {
        "type-slot".hash(state);
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
      (Self::Record(a), Self::Record(b)) => a.name.cmp(&b.name).then_with(|| a.fields.cmp(&b.fields)),
      (Self::Tuple(a), Self::Tuple(b)) => a.name().cmp(b.name()),
      (Self::Fn(a), Self::Fn(b)) => a
        .generics
        .cmp(&b.generics)
        .then_with(|| a.arg_types.cmp(&b.arg_types))
        .then_with(|| a.return_type.cmp(&b.return_type)),
      (Self::Set(a), Self::Set(b)) => a.cmp(b),
      (Self::Ref(a), Self::Ref(b)) => a.cmp(b),
      (Self::Variadic(a), Self::Variadic(b)) => a.cmp(b),
      (Self::Custom(a), Self::Custom(b)) => a.cmp(b),
      (Self::Optional(a), Self::Optional(b)) => a.cmp(b),
      (Self::Dynamic, Self::Dynamic) => Ordering::Equal,
      (Self::TypeVar(a), Self::TypeVar(b)) => a.cmp(b),
      (Self::TypeRef(a_name, a_args), Self::TypeRef(b_name, b_args)) => a_name.cmp(b_name).then_with(|| a_args.cmp(b_args)),
      (Self::Struct(a, _), Self::Struct(b, _)) => a.name.cmp(&b.name).then_with(|| a.fields.cmp(&b.fields)),
      (Self::Enum(a, _), Self::Enum(b, _)) => a.name().cmp(b.name()),
      (Self::Trait(a), Self::Trait(b)) => a.name.cmp(&b.name),
      (Self::TraitSet(a), Self::TraitSet(b)) => a.iter().map(|t| &t.name).cmp(b.iter().map(|t| &t.name)),
      _ => Ordering::Equal, // other variants already separated by kind order
    }
  }
}

/// Distinguishes fn-kind schemas (`:kind :fn`) from macro-kind schemas (`:kind :macro`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum SchemaKind {
  #[default]
  Fn,
  Macro,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalcitFnTypeAnnotation {
  pub generics: Arc<Vec<Arc<str>>>,
  pub where_bounds: Arc<Vec<CalcitGenericBound>>,
  pub arg_types: Vec<Arc<CalcitTypeAnnotation>>,
  pub return_type: Arc<CalcitTypeAnnotation>,
  /// Whether this schema was declared as `:kind :macro` (default: `:kind :fn`).
  pub fn_kind: SchemaKind,
  /// Rest-param type from `:rest` in the schema, if present.
  pub rest_type: Option<Arc<CalcitTypeAnnotation>>,
}

impl CalcitFnTypeAnnotation {
  fn where_bounds_to_edn(&self) -> Option<Edn> {
    if self.where_bounds.is_empty() {
      return None;
    }

    let mut map = EdnMapView::default();
    for bound in self.where_bounds.iter() {
      let value = if bound.traits.len() == 1 {
        Edn::Tag(bound.traits[0].name.clone())
      } else {
        Edn::List(EdnListView(
          bound.traits.iter().map(|trait_def| Edn::Tag(trait_def.name.clone())).collect(),
        ))
      };
      map.insert(Edn::Symbol(bound.name.clone()), value);
    }
    Some(Edn::Map(map))
  }

  pub(crate) fn validate_applied_type_args(&self) -> Result<(), String> {
    for arg in &self.arg_types {
      arg.validate_applied_type_args()?;
    }
    self.return_type.validate_applied_type_args()?;
    if let Some(rest) = &self.rest_type {
      rest.validate_applied_type_args()?;
    }
    Ok(())
  }

  fn to_inline_type_schema_edn(&self) -> Edn {
    let args: Vec<Edn> = self.arg_types.iter().map(|t| t.to_type_edn()).collect();
    let mut map = EdnMapView::default();
    if matches!(self.fn_kind, SchemaKind::Macro) {
      map.insert_key("kind", Edn::tag("macro"));
    }
    map.insert_key("args", Edn::List(EdnListView(args)));
    if !matches!(self.fn_kind, SchemaKind::Macro) || !matches!(self.return_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
      map.insert_key("return", self.return_type.to_type_edn());
    }
    if !self.generics.is_empty() {
      let generics: Vec<Edn> = self
        .generics
        .iter()
        .map(|s| Edn::Symbol(Arc::from(s.trim_start_matches('\''))))
        .collect();
      map.insert_key("generics", Edn::List(EdnListView(generics)));
    }
    if let Some(where_bounds) = self.where_bounds_to_edn() {
      map.insert_key("where", where_bounds);
    }
    if let Some(rest) = &self.rest_type {
      map.insert_key("rest", rest.to_type_edn());
    }
    Edn::Map(map)
  }

  /// Convert this fn-type annotation to a [`Calcit`] map value suitable for hint-fn injection.
  /// The result is compatible with `extract_schema_value` and `extract_arg_types_from_hint_form`.
  pub fn to_schema_calcit(&self) -> Calcit {
    CalcitTypeAnnotation::edn_type_to_calcit(&self.to_schema_edn())
  }

  /// Serialize this fn-type annotation to the schema [`Edn`] map format
  /// `{:kind :fn, :args [...], :return ..., :generics [...]}` used in `CodeEntry.schema`.
  pub fn to_schema_edn(&self) -> Edn {
    let args: Vec<Edn> = self.arg_types.iter().map(|t| t.to_type_edn()).collect();
    let mut map = EdnMapView::default();
    let kind_str = match self.fn_kind {
      SchemaKind::Fn => "fn",
      SchemaKind::Macro => "macro",
    };
    map.insert_key("kind", Edn::tag(kind_str));
    map.insert_key("args", Edn::List(EdnListView(args)));
    map.insert_key("return", self.return_type.to_type_edn());
    if !self.generics.is_empty() {
      let generics: Vec<Edn> = self
        .generics
        .iter()
        .map(|s| Edn::Symbol(Arc::from(s.trim_start_matches('\''))))
        .collect();
      map.insert_key("generics", Edn::List(EdnListView(generics)));
    }
    if let Some(where_bounds) = self.where_bounds_to_edn() {
      map.insert_key("where", where_bounds);
    }
    if let Some(rest) = &self.rest_type {
      map.insert_key("rest", rest.to_type_edn());
    }
    Edn::Map(map)
  }

  pub fn to_wrapped_schema_edn(&self) -> Edn {
    let args: Vec<Edn> = self.arg_types.iter().map(|t| t.to_type_edn()).collect();
    let mut map = EdnMapView::default();
    map.insert_key("args", Edn::List(EdnListView(args)));
    if !matches!(self.fn_kind, SchemaKind::Macro) || !matches!(self.return_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
      map.insert_key("return", self.return_type.to_type_edn());
    }
    if !self.generics.is_empty() {
      let generics: Vec<Edn> = self
        .generics
        .iter()
        .map(|s| Edn::Symbol(Arc::from(s.trim_start_matches('\''))))
        .collect();
      map.insert_key("generics", Edn::List(EdnListView(generics)));
    }
    if let Some(where_bounds) = self.where_bounds_to_edn() {
      map.insert_key("where", where_bounds);
    }
    if let Some(rest) = &self.rest_type {
      map.insert_key("rest", rest.to_type_edn());
    }

    let wrapped_tag = match self.fn_kind {
      SchemaKind::Fn => Edn::tag("fn"),
      SchemaKind::Macro => Edn::tag("macro"),
    };

    Edn::tuple(wrapped_tag, vec![Edn::Map(map)])
  }

  pub fn describe(&self) -> String {
    let generics = if self.generics.is_empty() {
      "".to_string()
    } else {
      let rendered = self.generics.iter().map(|name| format!("'{name}")).collect::<Vec<_>>().join(", ");
      format!("<{rendered}>")
    };
    let where_clause = if self.where_bounds.is_empty() {
      String::new()
    } else {
      let rendered = self
        .where_bounds
        .iter()
        .map(CalcitGenericBound::to_brief_string)
        .collect::<Vec<_>>()
        .join(", ");
      format!(" where {rendered}")
    };
    let args = if self.arg_types.is_empty() {
      "()".to_string()
    } else {
      let rendered = self.arg_types.iter().map(|t| t.describe()).collect::<Vec<_>>().join(", ");
      format!("({rendered})")
    };
    format!("fn{generics}{where_clause}{args} -> {}", self.return_type.describe())
  }

  pub fn render_signature_brief(&self) -> String {
    let generics = if self.generics.is_empty() {
      "".to_string()
    } else {
      let rendered = self.generics.iter().map(|name| format!("'{name}")).collect::<Vec<_>>().join(", ");
      format!("<{rendered}>")
    };
    let where_clause = if self.where_bounds.is_empty() {
      String::new()
    } else {
      let rendered = self
        .where_bounds
        .iter()
        .map(CalcitGenericBound::to_brief_string)
        .collect::<Vec<_>>()
        .join(", ");
      format!(" where {rendered}")
    };
    let args_repr = if self.arg_types.is_empty() {
      "()".to_string()
    } else {
      let parts = self.arg_types.iter().map(|t| t.to_brief_string()).collect::<Vec<_>>().join(", ");
      format!("({parts})")
    };

    format!("fn{generics}{where_clause}{args_repr} -> {}", self.return_type.to_brief_string())
  }

  pub fn matches_signature(&self, other: &CalcitFnTypeAnnotation) -> bool {
    if self.arg_types.len() != other.arg_types.len() {
      return false;
    }

    // Don't require generics count to match: actual concrete functions don't declare
    // generics even when expected fn type uses TypeVars. Bindings resolve TypeVars below.

    let mut bindings = TypeBindings::new();

    for (lhs, rhs) in self.arg_types.iter().zip(other.arg_types.iter()) {
      if !lhs.matches_with_bindings(rhs, &mut bindings) {
        return false;
      }
    }

    self.return_type.matches_with_bindings(other.return_type.as_ref(), &mut bindings)
  }
}

/// Check if a runtime `Calcit` value matches the expected `CalcitTypeAnnotation`.
/// Used for runtime type validation when creating records (`%{}`) and enum tuples (`%::`).
/// Returns `true` if the value is compatible with the declared type.
/// `Dynamic` types always match. `Nil` matches `Optional` types.
pub fn value_matches_type_annotation(value: &Calcit, expected: &CalcitTypeAnnotation) -> bool {
  match expected {
    CalcitTypeAnnotation::Dynamic => true,
    CalcitTypeAnnotation::Unit => matches!(value, Calcit::Nil),
    CalcitTypeAnnotation::Optional(inner) => matches!(value, Calcit::Nil) || value_matches_type_annotation(value, inner),
    CalcitTypeAnnotation::Bool => matches!(value, Calcit::Bool(_)),
    CalcitTypeAnnotation::Number => matches!(value, Calcit::Number(_)),
    CalcitTypeAnnotation::String => matches!(value, Calcit::Str(_)),
    CalcitTypeAnnotation::Symbol => matches!(value, Calcit::Symbol { .. }),
    CalcitTypeAnnotation::Tag => matches!(value, Calcit::Tag(_)),
    CalcitTypeAnnotation::List(_) => matches!(value, Calcit::List(_)),
    CalcitTypeAnnotation::Map(_, _) => matches!(value, Calcit::Map(_)),
    CalcitTypeAnnotation::Set(_) => matches!(value, Calcit::Set(_)),
    CalcitTypeAnnotation::Ref(_) => matches!(value, Calcit::Ref(..)),
    CalcitTypeAnnotation::Buffer => matches!(value, Calcit::Buffer(_)),
    CalcitTypeAnnotation::CirruQuote => matches!(value, Calcit::CirruQuote(_)),
    CalcitTypeAnnotation::DynTuple => matches!(value, Calcit::Tuple(_)),
    CalcitTypeAnnotation::DynFn | CalcitTypeAnnotation::Fn(_) => matches!(value, Calcit::Fn { .. } | Calcit::Proc(_)),
    CalcitTypeAnnotation::Struct(expected_struct, _) => match value {
      Calcit::Struct(s) => s.name == expected_struct.name,
      Calcit::Record(r) => r.struct_ref.name == expected_struct.name,
      _ => false,
    },
    CalcitTypeAnnotation::Enum(expected_enum, _) => match value {
      Calcit::Enum(e) => e.name() == expected_enum.name(),
      Calcit::Tuple(t) => t.sum_type.as_ref().is_some_and(|st| st.name() == expected_enum.name()),
      _ => false,
    },
    CalcitTypeAnnotation::TypeRef(expected_name, _) => match value {
      Calcit::Struct(s) => CalcitTypeAnnotation::type_ref_name_matches(expected_name, s.name.ref_str()),
      Calcit::Record(r) => CalcitTypeAnnotation::type_ref_name_matches(expected_name, r.struct_ref.name.ref_str()),
      Calcit::Enum(e) => CalcitTypeAnnotation::type_ref_name_matches(expected_name, e.name().ref_str()),
      Calcit::Tuple(t) => t
        .sum_type
        .as_ref()
        .is_some_and(|st| CalcitTypeAnnotation::type_ref_name_matches(expected_name, st.name().ref_str())),
      _ => false,
    },
    CalcitTypeAnnotation::Record(expected_struct) => match value {
      Calcit::Record(r) => r.struct_ref.name == expected_struct.name,
      _ => false,
    },
    CalcitTypeAnnotation::Tuple(expected_enum) => match value {
      Calcit::Tuple(t) => t.sum_type.as_ref().is_some_and(|st| st.name() == expected_enum.name()),
      _ => false,
    },
    CalcitTypeAnnotation::Trait(expected_trait) => match value {
      Calcit::Record(r) => r.struct_ref.impls.iter().any(|imp| imp.name() == &expected_trait.name),
      Calcit::Tuple(t) => t.impls().iter().any(|imp| imp.name() == &expected_trait.name),
      _ => false,
    },
    CalcitTypeAnnotation::TraitSet(traits) => match value {
      Calcit::Record(r) => traits.iter().all(|t| r.struct_ref.impls.iter().any(|imp| imp.name() == &t.name)),
      Calcit::Tuple(t) => traits.iter().all(|tr| t.impls().iter().any(|imp| imp.name() == &tr.name)),
      _ => false,
    },
    CalcitTypeAnnotation::Custom(custom) => match custom.as_ref() {
      Calcit::Tag(tag) => match tag.ref_str() {
        "any" => true,
        "nil" => matches!(value, Calcit::Nil),
        "record" => matches!(value, Calcit::Record(_)),
        "struct" => matches!(value, Calcit::Struct(_)),
        "enum" => matches!(value, Calcit::Enum(_)),
        _ => true, // unknown custom types: be permissive
      },
      _ => true,
    },
    // Generic type variables cannot be checked at runtime; allow any value
    CalcitTypeAnnotation::TypeVar(_) => true,
    CalcitTypeAnnotation::Variadic(inner) => matches!(value, Calcit::List(_)) || value_matches_type_annotation(value, inner),
    CalcitTypeAnnotation::TypeSlot(name) => {
      if let Some(resolved) = resolve_type_slot(name) {
        value_matches_type_annotation(value, &resolved)
      } else {
        true // unbound slot: permissive like Dynamic
      }
    }
  }
}

/// Return a brief human-readable type name for a runtime `Calcit` value, used in error messages.
pub fn brief_type_of_value(value: &Calcit) -> &'static str {
  match value {
    Calcit::Nil => "nil",
    Calcit::Bool(_) => "bool",
    Calcit::Number(_) => "number",
    Calcit::Str(_) => "string",
    Calcit::Symbol { .. } | Calcit::Local { .. } | Calcit::Import { .. } => "symbol",
    Calcit::Tag(_) => "tag",
    Calcit::List(_) => "list",
    Calcit::Map(_) => "map",
    Calcit::Set(_) => "set",
    Calcit::Ref(..) => "ref",
    Calcit::Buffer(_) => "buffer",
    Calcit::CirruQuote(_) => "cirru-quote",
    Calcit::Tuple(_) => "tuple",
    Calcit::Record(_) => "record",
    Calcit::Struct(_) => "struct",
    Calcit::Enum(_) => "enum",
    Calcit::Fn { .. } | Calcit::Proc(_) => "fn",
    Calcit::Macro { .. } => "macro",
    Calcit::Syntax(..) => "syntax",
    Calcit::Method(..) => "method",
    Calcit::Trait(_) => "trait",
    Calcit::Impl(_) => "impl",
    _ => "unknown",
  }
}
