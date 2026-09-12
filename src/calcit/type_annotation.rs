use std::{
  cell::{Cell, RefCell},
  cmp::Ordering,
  collections::{HashMap, HashSet},
  fmt,
  hash::{Hash, Hasher},
  sync::Arc,
};

use std::thread_local;

use cirru_edn::{Edn, EdnListView, EdnMapView, EdnSetView, EdnTag};

use super::{
  CORE_NS, Calcit, CalcitEnumDef, CalcitEnumValue, CalcitFn, CalcitImpl, CalcitImport, CalcitList, CalcitProc, CalcitStructDef,
  CalcitStructValue, CalcitSymbolInfo, CalcitSyntax, CalcitTrait,
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
  /// A slot is declared via `deftype-slot`; the optional value is retained for legacy snapshots.
  static TYPE_SLOTS: RefCell<HashMap<Arc<str>, Option<Arc<CalcitTypeAnnotation>>>> = RefCell::new(HashMap::new());
  /// Entry-level type-slot bindings loaded from the selected `entries.<name>.type-slots` before preprocessing begins.
  static ENTRY_TYPE_SLOTS: RefCell<HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>> = RefCell::new(HashMap::new());
  /// Scoped type-slot overrides for `with-type-slot` blocks.
  /// Each entry is a stack; the top value shadows the base `TYPE_SLOTS` binding within the scope.
  static TYPE_SLOT_OVERRIDES: RefCell<HashMap<Arc<str>, Vec<Arc<CalcitTypeAnnotation>>>> = RefCell::new(HashMap::new());
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

fn current_type_annotation_namespace() -> Option<Arc<str>> {
  current_type_annotation_warning_context().and_then(|label| label.rsplit_once('/').map(|(namespace, _)| Arc::from(namespace)))
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

/// Replace entry-level type-slot bindings with a validated configuration map.
/// Values are full `namespace/definition` paths, or `:dynamic` for an explicit opt-out.
pub fn configure_entry_type_slots(bindings: &HashMap<String, String>) -> Result<(), String> {
  let mut configured: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::with_capacity(bindings.len());
  for (raw_name, raw_type_path) in bindings {
    let name = raw_name.trim().trim_start_matches(':');
    if name.is_empty() {
      return Err("type slot name cannot be empty".to_owned());
    }

    let type_path = raw_type_path.trim();
    let annotation = if matches!(type_path, ":dynamic" | "dynamic") {
      crate::calcit::DYNAMIC_TYPE.clone()
    } else {
      let Some((ns, def)) = type_path.rsplit_once('/') else {
        return Err(format!(
          "type slot `{name}` expected a full `namespace/definition` type path, got `{type_path}`"
        ));
      };
      if ns.is_empty() || def.is_empty() {
        return Err(format!(
          "type slot `{name}` expected a full `namespace/definition` type path, got `{type_path}`"
        ));
      }
      Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from(type_path), Arc::new(vec![])))
    };
    configured.insert(Arc::from(name), annotation);
  }

  ENTRY_TYPE_SLOTS.with(|slots| *slots.borrow_mut() = configured);
  Ok(())
}

/// Look up the type bound to a slot.
/// Scoped `with-type-slot` overrides take priority, followed by the selected entry configuration.
pub fn resolve_type_slot(name: &str) -> Option<Arc<CalcitTypeAnnotation>> {
  // Check scoped overrides first (innermost wins).
  let override_val = TYPE_SLOT_OVERRIDES.with(|overrides| overrides.borrow().get(name).and_then(|stack| stack.last().cloned()));
  if override_val.is_some() {
    return override_val;
  }
  let entry_value = ENTRY_TYPE_SLOTS.with(|slots| slots.borrow().get(name).cloned());
  if entry_value.is_some() {
    return entry_value;
  }
  TYPE_SLOTS.with(|slots| slots.borrow().get(name).and_then(|v| v.clone()))
}

/// Push a scoped type override for `with-type-slot`. Must be paired with `pop_type_slot_override`.
pub fn push_type_slot_override(name: Arc<str>, ty: Arc<CalcitTypeAnnotation>) {
  TYPE_SLOT_OVERRIDES.with(|overrides| {
    overrides.borrow_mut().entry(name).or_default().push(ty);
  });
}

/// Pop the innermost scoped override for `name`. Cleans up empty stacks.
pub fn pop_type_slot_override(name: &str) {
  TYPE_SLOT_OVERRIDES.with(|overrides| {
    let mut map = overrides.borrow_mut();
    if let Some(stack) = map.get_mut(name) {
      stack.pop();
      if stack.is_empty() {
        map.remove(name);
      }
    }
  });
}

/// Clear all type slots. Called at program startup/shutdown to avoid stale state across runs.
#[allow(dead_code)]
pub fn clear_type_slots() {
  TYPE_SLOTS.with(|slots| slots.borrow_mut().clear());
  ENTRY_TYPE_SLOTS.with(|slots| slots.borrow_mut().clear());
  TYPE_SLOT_OVERRIDES.with(|overrides| overrides.borrow_mut().clear());
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
  /// TypeRef arity validation may parse the same nominal definition that is
  /// currently being built (for example, an enum containing itself). Keep the
  /// recursive edge symbolic while still validating the outer application.
  static TYPE_REF_ARITY_RESOLUTION_STACK: RefCell<HashSet<Arc<str>>> = RefCell::new(HashSet::new());
  /// Alias and slot expansion are relation-local. These stacks prevent a
  /// malformed alias graph from recursively re-entering the same symbolic
  /// edge while keeping independent compatibility checks isolated.
  static TYPE_RELATION_SLOT_STACK: RefCell<HashSet<Arc<str>>> = RefCell::new(HashSet::new());
  static COMPATIBILITY_RELATION_VISITS: Cell<Option<usize>> = const { Cell::new(None) };
}

pub static DYNAMIC_TYPE: LazyLock<Arc<CalcitTypeAnnotation>> = LazyLock::new(|| Arc::new(CalcitTypeAnnotation::Dynamic));

pub(crate) type TypeBindings = HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>;

const TYPE_DIAGNOSTIC_DEPTH_LIMIT: usize = 32;

struct CompatibilityRelationGuard {
  owns_context: bool,
}

impl Drop for CompatibilityRelationGuard {
  fn drop(&mut self) {
    if self.owns_context {
      COMPATIBILITY_RELATION_VISITS.with(|visits| visits.set(None));
    }
  }
}

fn enter_compatibility_relation() -> CompatibilityRelationGuard {
  let owns_context = COMPATIBILITY_RELATION_VISITS.with(|visits| {
    if visits.get().is_none() {
      visits.set(Some(0));
      true
    } else {
      false
    }
  });
  CompatibilityRelationGuard { owns_context }
}

fn consume_compatibility_relation_visit() -> bool {
  COMPATIBILITY_RELATION_VISITS.with(|visits| {
    let next = visits.get().unwrap_or(0).saturating_add(1);
    visits.set(Some(next));
    next <= TYPE_RELATION_NODE_LIMIT
  })
}

fn with_type_relation_symbol<T>(
  stack: &'static std::thread::LocalKey<RefCell<HashSet<Arc<str>>>>,
  name: &str,
  f: impl FnOnce() -> T,
) -> Option<T> {
  struct SymbolGuard {
    stack: &'static std::thread::LocalKey<RefCell<HashSet<Arc<str>>>>,
    key: Arc<str>,
  }

  impl Drop for SymbolGuard {
    fn drop(&mut self) {
      self.stack.with(|active| {
        active.borrow_mut().remove(&self.key);
      });
    }
  }

  let key: Arc<str> = Arc::from(name.trim_start_matches('\'').trim_start_matches(':'));
  let entered = stack.with(|active| active.borrow_mut().insert(key.clone()));
  if !entered {
    return None;
  }
  let _guard = SymbolGuard { stack, key };
  Some(f())
}

fn type_ref_arity_resolution_key(name: &str) -> Arc<str> {
  let stripped = name.trim_start_matches('\'').trim_start_matches(':');
  if stripped.contains('/') {
    Arc::from(stripped)
  } else if let Some(namespace) = current_type_annotation_namespace() {
    Arc::from(format!("{namespace}/{stripped}"))
  } else {
    Arc::from(stripped)
  }
}

/// Compare the static data-schema identity of annotations without allowing
/// attached struct/enum impls to change nominal identity. Unlike the display
/// EDN form, this preserves qualified nested nominal definitions. Traversal is
/// iterative and pair-memoized so deep schemas and shared subgraphs are finite.
pub(crate) fn same_data_schema_annotation<'a>(actual: &'a CalcitTypeAnnotation, expected: &'a CalcitTypeAnnotation) -> bool {
  use CalcitTypeAnnotation as Type;

  fn push_slices<'a>(worklist: &mut Vec<(&'a Type, &'a Type)>, actual: &'a [Arc<Type>], expected: &'a [Arc<Type>]) -> bool {
    if actual.len() != expected.len() {
      return false;
    }
    worklist.extend(
      actual
        .iter()
        .zip(expected.iter())
        .map(|(actual, expected)| (actual.as_ref(), expected.as_ref())),
    );
    true
  }

  fn push_struct<'a>(worklist: &mut Vec<(&'a Type, &'a Type)>, actual: &'a CalcitStructDef, expected: &'a CalcitStructDef) -> bool {
    if actual.definition_ref.is_none()
      || actual.definition_ref != expected.definition_ref
      || actual.name != expected.name
      || actual.fields != expected.fields
      || actual.generics != expected.generics
      || actual.where_bounds != expected.where_bounds
    {
      return false;
    }
    push_slices(worklist, actual.field_types.as_ref(), expected.field_types.as_ref())
  }

  fn push_enum<'a>(worklist: &mut Vec<(&'a Type, &'a Type)>, actual: &'a CalcitEnumDef, expected: &'a CalcitEnumDef) -> bool {
    if actual.definition_ref().is_none()
      || actual.definition_ref() != expected.definition_ref()
      || actual.name() != expected.name()
      || actual.generics() != expected.generics()
      || actual.where_bounds() != expected.where_bounds()
      || actual.variants().len() != expected.variants().len()
    {
      return false;
    }
    for (actual, expected) in actual.variants().iter().zip(expected.variants().iter()) {
      if actual.tag != expected.tag || !push_slices(worklist, actual.payload_types(), expected.payload_types()) {
        return false;
      }
    }
    true
  }

  fn push_macro_syntax<'a>(
    worklist: &mut Vec<(&'a Type, &'a Type)>,
    actual: &'a MacroSyntaxType,
    expected: &'a MacroSyntaxType,
  ) -> bool {
    match (actual, expected) {
      (MacroSyntaxType::Expr(actual), MacroSyntaxType::Expr(expected)) => {
        worklist.push((actual.as_ref(), expected.as_ref()));
        true
      }
      _ => actual == expected,
    }
  }

  let mut worklist = vec![(actual, expected)];
  let mut visited = HashSet::new();
  let mut visits = 0usize;
  while let Some((actual, expected)) = worklist.pop() {
    if !visited.insert((actual as *const Type as usize, expected as *const Type as usize)) {
      continue;
    }
    visits += 1;
    if visits > TYPE_RELATION_NODE_LIMIT {
      return false;
    }

    let matches = match (actual, expected) {
      (Type::List(actual), Type::List(expected))
      | (Type::Set(actual), Type::Set(expected))
      | (Type::Ref(actual), Type::Ref(expected))
      | (Type::Variadic(actual), Type::Variadic(expected))
      | (Type::Optional(actual), Type::Optional(expected))
      | (Type::JsNullish(actual), Type::JsNullish(expected)) => {
        worklist.push((actual.as_ref(), expected.as_ref()));
        true
      }
      (Type::Map(actual_key, actual_value), Type::Map(expected_key, expected_value)) => {
        worklist.push((actual_key.as_ref(), expected_key.as_ref()));
        worklist.push((actual_value.as_ref(), expected_value.as_ref()));
        true
      }
      (Type::Struct(actual, actual_args), Type::Struct(expected, expected_args)) => {
        push_struct(&mut worklist, actual, expected) && push_slices(&mut worklist, actual_args, expected_args)
      }
      (Type::Enum(actual, actual_args), Type::Enum(expected, expected_args)) => {
        push_enum(&mut worklist, actual, expected) && push_slices(&mut worklist, actual_args, expected_args)
      }
      (Type::StructValue(actual), Type::StructValue(expected)) | (Type::StructDef(actual), Type::StructDef(expected)) => {
        push_struct(&mut worklist, actual, expected)
      }
      (Type::EnumValue(actual), Type::EnumValue(expected)) | (Type::EnumDef(actual), Type::EnumDef(expected)) => {
        push_enum(&mut worklist, actual, expected)
      }
      (Type::TypeRef(actual_name, actual_args), Type::TypeRef(expected_name, expected_args)) => {
        actual_name == expected_name && push_slices(&mut worklist, actual_args, expected_args)
      }
      (Type::Fn(actual), Type::Fn(expected)) => {
        actual.generics == expected.generics
          && actual.where_bounds == expected.where_bounds
          && actual.fn_kind == expected.fn_kind
          && actual.features == expected.features
          && push_slices(&mut worklist, &actual.arg_types, &expected.arg_types)
          && match (&actual.rest_type, &expected.rest_type) {
            (Some(actual), Some(expected)) => {
              worklist.push((actual.as_ref(), expected.as_ref()));
              true
            }
            (None, None) => true,
            _ => false,
          }
          && {
            worklist.push((actual.return_type.as_ref(), expected.return_type.as_ref()));
            true
          }
      }
      (Type::Macro(actual), Type::Macro(expected)) => {
        if actual.generics != expected.generics
          || actual.where_bounds != expected.where_bounds
          || actual.capabilities != expected.capabilities
          || actual.features != expected.features
          || actual.required_inputs.len() != expected.required_inputs.len()
          || actual.optional_inputs.len() != expected.optional_inputs.len()
        {
          false
        } else {
          let required_match = actual
            .required_inputs
            .iter()
            .zip(expected.required_inputs.iter())
            .all(|(actual, expected)| push_macro_syntax(&mut worklist, actual, expected));
          let optional_match = actual
            .optional_inputs
            .iter()
            .zip(expected.optional_inputs.iter())
            .all(|(actual, expected)| push_macro_syntax(&mut worklist, actual, expected));
          let rest_match = match (&actual.rest_input, &expected.rest_input) {
            (Some(actual), Some(expected)) => push_macro_syntax(&mut worklist, actual, expected),
            (None, None) => true,
            _ => false,
          };
          let expansion_match = match (&actual.expansion, &expected.expansion) {
            (MacroExpansionType::Expr(actual), MacroExpansionType::Expr(expected))
            | (MacroExpansionType::Definition(actual), MacroExpansionType::Definition(expected)) => {
              worklist.push((actual.as_ref(), expected.as_ref()));
              true
            }
            (actual, expected) => actual == expected,
          };
          required_match && optional_match && rest_match && expansion_match
        }
      }
      (Type::Syntax(actual), Type::Syntax(expected)) => push_macro_syntax(&mut worklist, actual, expected),
      _ => actual == expected,
    };
    if !matches {
      return false;
    }
  }
  true
}

/// Result of asking whether an annotation is strong enough to authorize
/// static inference or unchecked lowering. Compatibility may still accept a
/// boundary that proof deliberately refuses to cross.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeProof {
  Proven,
  NeedsBoundary(TypeBoundaryReason),
  Mismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeBoundaryReason {
  Dynamic,
  LegacyAny,
  UnknownCallable,
  TagCallable,
  UnresolvedTypeSlot,
  LegacyNullish,
  ErasedTypeArguments,
  RecursiveTypeVariable,
  RecursiveTypeAlias,
  RecursiveTypeSlot,
  UnresolvedNominalIdentity,
  TypeComplexityLimit,
}

const TYPE_RELATION_NODE_LIMIT: usize = 16_384;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TypeRelationStats {
  node_visits: usize,
  memo_hits: usize,
  max_worklist: usize,
  max_depth: usize,
}

#[derive(Debug, Clone, Copy)]
enum PlainRelationMode {
  Compatibility,
  Proof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AliasRelationKey {
  alias: Arc<str>,
  other: usize,
  alias_on_actual_side: bool,
  proof: bool,
}

thread_local! {
  static TYPE_ALIAS_RELATION_STACK: RefCell<Vec<AliasRelationKey>> = const { RefCell::new(vec![]) };
}

struct AliasRelationGuard;

impl Drop for AliasRelationGuard {
  fn drop(&mut self) {
    TYPE_ALIAS_RELATION_STACK.with(|stack| {
      stack.borrow_mut().pop();
    });
  }
}

/// Guard schema-alias expansion within one relation. The key includes relation
/// direction and the other type node, so legitimate repeated aliases in
/// sibling branches remain independent while pure expansion cycles terminate.
fn enter_alias_relation(
  alias: &Arc<str>,
  other: &CalcitTypeAnnotation,
  alias_on_actual_side: bool,
  proof: bool,
) -> Result<AliasRelationGuard, TypeBoundaryReason> {
  let key = AliasRelationKey {
    alias: alias.clone(),
    other: other as *const CalcitTypeAnnotation as usize,
    alias_on_actual_side,
    proof,
  };
  TYPE_ALIAS_RELATION_STACK.with(|stack| {
    let mut stack = stack.borrow_mut();
    if stack.contains(&key) {
      return Err(TypeBoundaryReason::RecursiveTypeAlias);
    }
    if stack.len() >= 256 {
      return Err(TypeBoundaryReason::TypeComplexityLimit);
    }
    stack.push(key);
    Ok(AliasRelationGuard)
  })
}

/// Iterative fast path for the high-volume structural part of type relations.
///
/// Returning `None` hands uncommon semantic cases (traits, callable variance,
/// aliases, and generic binding) to the existing matcher. Supported shapes use
/// a per-relation pair memo and a hard node budget, so deep container/nominal
/// arguments and shared DAGs do not consume the Rust call stack.
fn bounded_plain_type_relation<'a>(
  actual: &'a CalcitTypeAnnotation,
  expected: &'a CalcitTypeAnnotation,
  mode: PlainRelationMode,
) -> Result<Option<(TypeProof, TypeRelationStats)>, TypeRelationStats> {
  use CalcitTypeAnnotation as Type;
  use TypeBoundaryReason as Boundary;
  use TypeProof::{Mismatch, NeedsBoundary, Proven};

  fn push_pair<'a>(
    worklist: &mut Vec<(&'a CalcitTypeAnnotation, &'a CalcitTypeAnnotation, usize)>,
    left: &'a Arc<CalcitTypeAnnotation>,
    right: &'a Arc<CalcitTypeAnnotation>,
    depth: usize,
  ) {
    worklist.push((left.as_ref(), right.as_ref(), depth + 1));
  }

  fn is_plain_mismatch_node(value: &CalcitTypeAnnotation) -> bool {
    matches!(
      value,
      CalcitTypeAnnotation::Bool
        | CalcitTypeAnnotation::Number
        | CalcitTypeAnnotation::String
        | CalcitTypeAnnotation::Symbol
        | CalcitTypeAnnotation::Buffer
        | CalcitTypeAnnotation::F64Buffer
        | CalcitTypeAnnotation::CirruQuote
        | CalcitTypeAnnotation::JsObject
        | CalcitTypeAnnotation::Nil
        | CalcitTypeAnnotation::Unit
        | CalcitTypeAnnotation::List(_)
        | CalcitTypeAnnotation::Map(_, _)
        | CalcitTypeAnnotation::Set(_)
        | CalcitTypeAnnotation::Ref(_)
        | CalcitTypeAnnotation::Variadic(_)
    )
  }

  let mut worklist = vec![(actual, expected, 0usize)];
  let mut visited: HashSet<(usize, usize)> = HashSet::new();
  let mut result = Proven;
  let mut stats = TypeRelationStats {
    max_worklist: 1,
    ..TypeRelationStats::default()
  };

  while let Some((actual, expected, depth)) = worklist.pop() {
    stats.max_depth = stats.max_depth.max(depth);
    let key = (actual as *const Type as usize, expected as *const Type as usize);
    if !visited.insert(key) {
      stats.memo_hits += 1;
      continue;
    }
    stats.node_visits += 1;
    if stats.node_visits > TYPE_RELATION_NODE_LIMIT {
      return Err(stats);
    }
    if matches!(mode, PlainRelationMode::Compatibility) && !consume_compatibility_relation_visit() {
      return Err(stats);
    }

    match (actual, expected) {
      (_, Type::Dynamic) | (Type::Dynamic, _) => {
        if matches!(mode, PlainRelationMode::Proof) {
          result = result.and(NeedsBoundary(Boundary::Dynamic));
        }
      }
      (Type::TypeVar(left), Type::TypeVar(right)) if left == right => {}
      (Type::List(left), Type::List(right))
      | (Type::Set(left), Type::Set(right))
      | (Type::Variadic(left), Type::Variadic(right))
      | (Type::Optional(left), Type::Optional(right))
      | (Type::JsNullish(left), Type::JsNullish(right)) => push_pair(&mut worklist, left, right, depth),
      (Type::Ref(left), Type::Ref(right)) => {
        push_pair(&mut worklist, left, right, depth);
        if matches!(mode, PlainRelationMode::Proof) {
          worklist.push((right.as_ref(), left.as_ref(), depth + 1));
        }
      }
      (Type::Map(actual_key, actual_value), Type::Map(expected_key, expected_value)) => {
        push_pair(&mut worklist, actual_key, expected_key, depth);
        push_pair(&mut worklist, actual_value, expected_value, depth);
      }
      (Type::TypeRef(actual_name, actual_args), Type::TypeRef(expected_name, expected_args)) => {
        match Type::type_ref_nominal_match(actual_name, expected_name) {
          Some(true) => {}
          Some(false) => return Ok(Some((Mismatch, stats))),
          None => return Ok(None),
        }
        if actual_args.is_empty() != expected_args.is_empty() {
          match mode {
            PlainRelationMode::Compatibility => continue,
            PlainRelationMode::Proof => {
              result = result.and(NeedsBoundary(Boundary::ErasedTypeArguments));
              continue;
            }
          }
        }
        if actual_args.len() != expected_args.len() {
          return Ok(Some((Mismatch, stats)));
        }
        for (left, right) in actual_args.iter().zip(expected_args.iter()) {
          push_pair(&mut worklist, left, right, depth);
        }
      }
      (Type::Struct(actual_def, actual_args), Type::Struct(expected_def, expected_args)) => {
        let nominal_match = if Arc::ptr_eq(actual_def, expected_def) {
          Some(true)
        } else {
          Type::struct_nominal_match(actual_def, expected_def)
        };
        match nominal_match {
          Some(true) => {}
          Some(false) => return Ok(Some((Mismatch, stats))),
          None => return Ok(None),
        }
        if actual_args.is_empty() != expected_args.is_empty() {
          match mode {
            PlainRelationMode::Compatibility => return Ok(None),
            PlainRelationMode::Proof => {
              result = result.and(NeedsBoundary(Boundary::ErasedTypeArguments));
              continue;
            }
          }
        }
        if actual_args.len() != expected_args.len() {
          return Ok(Some((Mismatch, stats)));
        }
        for (left, right) in actual_args.iter().zip(expected_args.iter()) {
          push_pair(&mut worklist, left, right, depth);
        }
      }
      (Type::Enum(actual_def, actual_args), Type::Enum(expected_def, expected_args)) => {
        let nominal_match = if Arc::ptr_eq(actual_def, expected_def) {
          Some(true)
        } else {
          Type::enum_nominal_match(actual_def, expected_def)
        };
        match nominal_match {
          Some(true) => {}
          Some(false) => return Ok(Some((Mismatch, stats))),
          None => return Ok(None),
        }
        if actual_args.is_empty() != expected_args.is_empty() {
          match mode {
            PlainRelationMode::Compatibility => continue,
            PlainRelationMode::Proof => {
              result = result.and(NeedsBoundary(Boundary::ErasedTypeArguments));
              continue;
            }
          }
        }
        if actual_args.len() != expected_args.len() {
          return Ok(Some((Mismatch, stats)));
        }
        for (left, right) in actual_args.iter().zip(expected_args.iter()) {
          push_pair(&mut worklist, left, right, depth);
        }
      }
      (Type::Bool, Type::Bool)
      | (Type::Number, Type::Number)
      | (Type::String, Type::String)
      | (Type::Symbol, Type::Symbol)
      | (Type::Tag, Type::Tag)
      | (Type::DynFn, Type::DynFn)
      | (Type::Buffer, Type::Buffer)
      | (Type::F64Buffer, Type::F64Buffer)
      | (Type::CirruQuote, Type::CirruQuote)
      | (Type::JsObject, Type::JsObject)
      | (Type::AnonymousEnum, Type::AnonymousEnum)
      | (Type::Nil, Type::Nil)
      | (Type::Unit, Type::Unit) => {}
      (Type::Nil, Type::Optional(_)) | (Type::Nil, Type::JsNullish(_)) => {
        if matches!(mode, PlainRelationMode::Proof) {
          result = result.and(NeedsBoundary(Boundary::LegacyNullish));
        }
      }
      (plain, Type::Optional(inner)) if !matches!(plain, Type::Optional(_) | Type::JsNullish(_)) => {
        if matches!(mode, PlainRelationMode::Proof) {
          result = result.and(NeedsBoundary(Boundary::LegacyNullish));
        }
        worklist.push((plain, inner.as_ref(), depth + 1));
      }
      (plain, Type::JsNullish(inner)) if !matches!(plain, Type::Optional(_) | Type::JsNullish(_)) => {
        if matches!(mode, PlainRelationMode::Proof) {
          result = result.and(NeedsBoundary(Boundary::LegacyNullish));
        }
        worklist.push((plain, inner.as_ref(), depth + 1));
      }
      (Type::Fn(_), _)
      | (_, Type::Fn(_))
      | (Type::Macro(_), _)
      | (_, Type::Macro(_))
      | (Type::Syntax(_), _)
      | (_, Type::Syntax(_))
      | (Type::Custom(_), _)
      | (_, Type::Custom(_))
      | (Type::Trait(_), _)
      | (_, Type::Trait(_))
      | (Type::TraitSet(_), _)
      | (_, Type::TraitSet(_))
      | (Type::TypeSlot(_), _)
      | (_, Type::TypeSlot(_))
      | (Type::StructDef(_), _)
      | (_, Type::StructDef(_))
      | (Type::EnumDef(_), _)
      | (_, Type::EnumDef(_))
      | (Type::StructValue(_), _)
      | (_, Type::StructValue(_))
      | (Type::EnumValue(_), _)
      | (_, Type::EnumValue(_))
      | (Type::TypeVar(_), _)
      | (_, Type::TypeVar(_))
      | (Type::TypeRef(_, _), _)
      | (_, Type::TypeRef(_, _)) => return Ok(None),
      _ if is_plain_mismatch_node(actual) && is_plain_mismatch_node(expected) => return Ok(Some((Mismatch, stats))),
      _ => return Ok(None),
    }
    stats.max_worklist = stats.max_worklist.max(worklist.len());
  }

  Ok(Some((result, stats)))
}

fn render_bounded_type_list(
  types: &[Arc<CalcitTypeAnnotation>],
  depth: usize,
  remaining: &mut usize,
  render: fn(&CalcitTypeAnnotation, usize, &mut usize) -> String,
) -> String {
  let mut parts = vec![];
  for annotation in types {
    if *remaining == 0 {
      parts.push("…".to_owned());
      break;
    }
    parts.push(render(annotation.as_ref(), depth, remaining));
  }
  parts.join(", ")
}

impl TypeProof {
  pub(crate) fn is_proven(self) -> bool {
    matches!(self, Self::Proven)
  }

  pub(crate) fn is_mismatch(self) -> bool {
    matches!(self, Self::Mismatch)
  }

  fn and(self, other: Self) -> Self {
    match (self, other) {
      (Self::Mismatch, _) | (_, Self::Mismatch) => Self::Mismatch,
      (Self::NeedsBoundary(reason), _) | (_, Self::NeedsBoundary(reason)) => Self::NeedsBoundary(reason),
      (Self::Proven, Self::Proven) => Self::Proven,
    }
  }
}

#[derive(Default)]
struct FnSchemaFields<'a> {
  has_any: bool,
  generics: Option<&'a Calcit>,
  where_clause: Option<&'a Calcit>,
  args: Option<&'a Calcit>,
  returns: Option<&'a Calcit>,
  rest: Option<&'a Calcit>,
  kind: Option<&'a Calcit>,
  features: Option<&'a Calcit>,
  async_invocation: Option<&'a Calcit>,
}

const ASYNC_INVOCATION_FEATURE: &str = "$async-invocation";

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
  /// A struct value's inferred type, identified by its struct definition.
  StructValue(Arc<CalcitStructDef>),
  /// An enum value's inferred type, identified by its enum definition.
  EnumValue(Arc<CalcitEnumDef>),
  /// An anonymous/open enum value whose closed variant set is unknown.
  AnonymousEnum,
  /// function type without a known signature
  DynFn,
  Fn(Arc<CalcitFnTypeAnnotation>),
  /// Compile-time macro contract. Unlike `Fn`, its inputs describe raw syntax
  /// and its output describes the semantic value produced after expansion.
  Macro(Arc<MacroSignature>),
  /// A macro-body local whose value is an unevaluated syntax node.
  Syntax(Arc<MacroSyntaxType>),
  /// Hashset type
  Set(Arc<CalcitTypeAnnotation>),
  Ref(Arc<CalcitTypeAnnotation>),
  Buffer,
  /// Immutable homogeneous f64 storage used by the strict Calx kernel boundary.
  F64Buffer,
  CirruQuote,
  /// Variadic parameter type constraint (for & args)
  Variadic(Arc<CalcitTypeAnnotation>),
  /// Fallback for shapes that are not yet modeled explicitly as a struct type
  Custom(Arc<Calcit>),
  /// No checking at static analaysis time
  Dynamic,
  /// Represents an type that can be nil or the given type
  Optional(Arc<CalcitTypeAnnotation>),
  /// JavaScript FFI value that may be `null`/`undefined` (represented as nil at runtime).
  /// This boundary type is intentionally distinct from both legacy Optional<T>
  /// and the nominal Calcit Option<T> enum.
  JsNullish(Arc<CalcitTypeAnnotation>),
  /// Struct type definition, optionally with applied generic arguments.
  /// `args` is empty when used as a bare type annotation (no generics applied).
  Struct(Arc<CalcitStructDef>, Arc<Vec<Arc<CalcitTypeAnnotation>>>),
  /// Enum type definition, optionally with applied generic arguments.
  Enum(Arc<CalcitEnumDef>, Arc<Vec<Arc<CalcitTypeAnnotation>>>),
  /// First-class definition value produced by `defstruct`.
  StructDef(Arc<CalcitStructDef>),
  /// First-class definition value produced by `defenum`.
  EnumDef(Arc<CalcitEnumDef>),
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
  /// Legacy nil sentinel. New APIs should prefer Option/Result or Unit.
  Nil,
  /// Unit type for side-effectful functions without a domain value.
  Unit,
  /// JavaScript FFI host value (opaque to the Calcit type system)
  JsObject,
  /// A type slot reference declared via `deftype-slot` and bound via `bind-type`.
  /// At type-checking time, this is resolved by looking up the global TYPE_SLOTS registry.
  TypeSlot(Arc<str>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MacroSyntaxType {
  Syntax,
  SyntaxSymbol,
  SyntaxList,
  Expr(Arc<CalcitTypeAnnotation>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MacroExpansionType {
  Dynamic,
  Expr(Arc<CalcitTypeAnnotation>),
  Definition(Arc<CalcitTypeAnnotation>),
  Declarations,
}

/// Effects that may be performed while a macro is expanding. These are
/// deliberately separate from ordinary function `:features`: capabilities
/// are an executable compile-time policy, not documentation or a runtime
/// backend marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MacroCapability {
  EnvRead,
  FsRead,
  PlatformRead,
  ClockRead,
  Log,
  MutableState,
  DynamicEval,
  FsWrite,
  Process,
  HostFfi,
}

impl MacroCapability {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::EnvRead => "env-read",
      Self::FsRead => "fs-read",
      Self::PlatformRead => "platform-read",
      Self::ClockRead => "clock-read",
      Self::Log => "log",
      Self::MutableState => "mutable-state",
      Self::DynamicEval => "dynamic-eval",
      Self::FsWrite => "fs-write",
      Self::Process => "process",
      Self::HostFfi => "host-ffi",
    }
  }

  pub fn parse(name: &str) -> Option<Self> {
    match name.trim_start_matches(':') {
      "env-read" => Some(Self::EnvRead),
      "fs-read" => Some(Self::FsRead),
      "platform-read" => Some(Self::PlatformRead),
      "clock-read" => Some(Self::ClockRead),
      "log" => Some(Self::Log),
      "mutable-state" => Some(Self::MutableState),
      "dynamic-eval" => Some(Self::DynamicEval),
      "fs-write" => Some(Self::FsWrite),
      "process" => Some(Self::Process),
      "host-ffi" => Some(Self::HostFfi),
      _ => None,
    }
  }

  /// Dangerous host mutations are intentionally unavailable to macro
  /// expansion. Keeping them in the model gives diagnostics and tooling a
  /// complete, auditable classification without turning declarations into an
  /// escape hatch.
  pub fn is_allowed(self) -> bool {
    !matches!(self, Self::FsWrite | Self::Process | Self::HostFfi)
  }

  pub fn is_cache_safe(self) -> bool {
    false
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroSignature {
  pub generics: Arc<Vec<Arc<str>>>,
  pub where_bounds: Arc<Vec<CalcitGenericBound>>,
  pub required_inputs: Arc<Vec<MacroSyntaxType>>,
  pub optional_inputs: Arc<Vec<MacroSyntaxType>>,
  pub rest_input: Option<MacroSyntaxType>,
  pub expansion: MacroExpansionType,
  pub capabilities: Arc<HashSet<MacroCapability>>,
  pub features: Arc<HashSet<EdnTag>>,
}

impl PartialOrd for MacroSignature {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for MacroSignature {
  fn cmp(&self, other: &Self) -> Ordering {
    let mut self_features = self.features.iter().map(EdnTag::ref_str).collect::<Vec<_>>();
    let mut other_features = other.features.iter().map(EdnTag::ref_str).collect::<Vec<_>>();
    self_features.sort_unstable();
    other_features.sort_unstable();
    self
      .generics
      .cmp(&other.generics)
      .then_with(|| self.where_bounds.cmp(&other.where_bounds))
      .then_with(|| self.required_inputs.cmp(&other.required_inputs))
      .then_with(|| self.optional_inputs.cmp(&other.optional_inputs))
      .then_with(|| self.rest_input.cmp(&other.rest_input))
      .then_with(|| self.expansion.cmp(&other.expansion))
      .then_with(|| {
        let mut a = self.capabilities.iter().copied().collect::<Vec<_>>();
        let mut b = other.capabilities.iter().copied().collect::<Vec<_>>();
        a.sort_unstable();
        b.sort_unstable();
        a.cmp(&b)
      })
      .then_with(|| self_features.cmp(&other_features))
  }
}

impl Hash for MacroSignature {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.generics.hash(state);
    self.where_bounds.hash(state);
    self.required_inputs.hash(state);
    self.optional_inputs.hash(state);
    self.rest_input.hash(state);
    self.expansion.hash(state);
    let mut capabilities = self.capabilities.iter().copied().collect::<Vec<_>>();
    capabilities.sort_unstable();
    capabilities.hash(state);
    let mut features = self.features.iter().map(EdnTag::ref_str).collect::<Vec<_>>();
    features.sort_unstable();
    features.hash(state);
  }
}

impl MacroSignature {
  /// Runtime macro signatures are phase-aware contracts. The method remains
  /// as a query helper for callers that report the contract mode.
  pub fn is_strict(&self) -> bool {
    true
  }

  /// Expansion caching may only trust macros whose compile-time execution has
  /// no declared effects. All runtime macros use this strict signature.
  pub fn is_cache_eligible(&self) -> bool {
    self.capabilities.iter().all(|capability| capability.is_cache_safe())
  }

  fn parse_contract(form: &Edn, generics: &[Arc<str>]) -> Option<MacroSyntaxType> {
    match form {
      Edn::Symbol(name) | Edn::Quote(cirru_parser::Cirru::Leaf(name)) => match name.as_ref() {
        "Syntax" => Some(MacroSyntaxType::Syntax),
        "SyntaxSymbol" => Some(MacroSyntaxType::SyntaxSymbol),
        "SyntaxList" => Some(MacroSyntaxType::SyntaxList),
        _ => None,
      },
      Edn::Enum(view) if view.variant.as_ref() == "Expr" && view.extra.len() == 1 => {
        let semantic = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(
          &CalcitTypeAnnotation::edn_type_to_calcit(&view.extra[0]),
          generics,
        );
        Some(MacroSyntaxType::Expr(semantic))
      }
      _ => None,
    }
  }

  fn parse_expansion(form: Option<&Edn>, generics: &[Arc<str>]) -> Option<MacroExpansionType> {
    let form = form?;
    match form {
      Edn::Symbol(name) | Edn::Quote(cirru_parser::Cirru::Leaf(name)) if name.as_ref() == "Declarations" => {
        Some(MacroExpansionType::Declarations)
      }
      Edn::Enum(view) if view.variant.as_ref() == "Declarations" && view.extra.is_empty() => Some(MacroExpansionType::Declarations),
      Edn::Enum(view) if matches!(view.variant.as_ref(), "Expr" | "Definition") && view.extra.len() == 1 => {
        let semantic = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(
          &CalcitTypeAnnotation::edn_type_to_calcit(&view.extra[0]),
          generics,
        );
        if view.variant.as_ref() == "Expr" {
          Some(MacroExpansionType::Expr(semantic))
        } else {
          Some(MacroExpansionType::Definition(semantic))
        }
      }
      _ => None,
    }
  }

  fn contract_to_edn(contract: &MacroSyntaxType) -> Edn {
    match contract {
      MacroSyntaxType::Syntax => Edn::Symbol(Arc::from("Syntax")),
      MacroSyntaxType::SyntaxSymbol => Edn::Symbol(Arc::from("SyntaxSymbol")),
      MacroSyntaxType::SyntaxList => Edn::Symbol(Arc::from("SyntaxList")),
      MacroSyntaxType::Expr(semantic) => Edn::enum_value("Expr", vec![semantic.to_type_edn()]),
    }
  }

  fn expansion_to_edn(expansion: &MacroExpansionType) -> Option<Edn> {
    match expansion {
      MacroExpansionType::Dynamic => None,
      MacroExpansionType::Expr(semantic) => Some(Edn::enum_value("Expr", vec![semantic.to_type_edn()])),
      MacroExpansionType::Definition(semantic) => Some(Edn::enum_value("Definition", vec![semantic.to_type_edn()])),
      MacroExpansionType::Declarations => Some(Edn::enum_value("Declarations", vec![])),
    }
  }

  pub fn to_wrapped_schema_edn(&self) -> Edn {
    let mut map = EdnMapView::default();
    map.insert_key(
      "required",
      Edn::List(EdnListView(self.required_inputs.iter().map(Self::contract_to_edn).collect())),
    );
    if !self.optional_inputs.is_empty() {
      map.insert_key(
        "optional",
        Edn::List(EdnListView(self.optional_inputs.iter().map(Self::contract_to_edn).collect())),
      );
    }
    if let Some(rest) = &self.rest_input {
      map.insert_key("rest", Self::contract_to_edn(rest));
    }
    if let Some(expansion) = Self::expansion_to_edn(&self.expansion) {
      map.insert_key("expansion", expansion);
    }
    let mut set = EdnSetView::default();
    let mut capabilities = self.capabilities.iter().copied().collect::<Vec<_>>();
    capabilities.sort_unstable();
    for capability in capabilities {
      set.insert(Edn::Tag(EdnTag::new(capability.as_str())));
    }
    map.insert_key("capabilities", Edn::Set(set));
    if !self.generics.is_empty() {
      map.insert_key(
        "generics",
        Edn::List(EdnListView(self.generics.iter().map(|name| Edn::Symbol(name.clone())).collect())),
      );
    }
    if let Some(where_bounds) = (CalcitFnTypeAnnotation {
      generics: self.generics.clone(),
      where_bounds: self.where_bounds.clone(),
      arg_types: vec![],
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      fn_kind: SchemaKind::Macro,
      rest_type: None,
      features: self.features.clone(),
    })
    .where_bounds_to_edn()
    {
      map.insert_key("where", where_bounds);
    }
    if !self.features.is_empty() {
      let mut set = EdnSetView::default();
      for feature in self.features.iter() {
        set.insert(Edn::Tag(feature.clone()));
      }
      map.insert_key("features", Edn::Set(set));
    }
    Edn::enum_value("Macro", vec![Edn::Map(map)])
  }
}

impl CalcitTypeAnnotation {
  /// Whether this annotation is a source reference to the nominal `calcit.core/Option` type.
  ///
  /// This intentionally does not treat legacy `Optional<T>` as `Option<T>`:
  /// the former uses `nil`, while the latter uses the named `:none` variant. A resolved enum
  /// does not retain its namespace, so it is deliberately excluded: a user enum named `Option`
  /// must not acquire core `Option`'s omitted-argument behavior.
  pub(crate) fn is_option_type(&self) -> bool {
    match self {
      Self::TypeRef(name, args) => {
        let name = name.trim_start_matches('\'').trim_start_matches(':');
        args.len() == 1 && matches!(name, "Option" | "calcit.core/Option")
      }
      Self::TypeSlot(name) => resolve_type_slot(name).is_some_and(|bound| bound.is_option_type()),
      _ => false,
    }
  }

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
      if !arg.compatible_with_bindings(var.as_ref(), bindings) {
        return false;
      }
    }
    true
  }

  pub(crate) fn validate_applied_type_args(&self) -> Result<(), String> {
    match self {
      Self::List(inner)
      | Self::Set(inner)
      | Self::Ref(inner)
      | Self::Variadic(inner)
      | Self::Optional(inner)
      | Self::JsNullish(inner) => inner.validate_applied_type_args(),
      Self::Map(key, value) => {
        key.validate_applied_type_args()?;
        value.validate_applied_type_args()
      }
      Self::Fn(signature) => signature.validate_applied_type_args(),
      Self::Macro(signature) => {
        for contract in signature.required_inputs.iter().chain(signature.optional_inputs.iter()) {
          if let MacroSyntaxType::Expr(semantic) = contract {
            semantic.validate_applied_type_args()?;
          }
        }
        if let Some(MacroSyntaxType::Expr(semantic)) = &signature.rest_input {
          semantic.validate_applied_type_args()?;
        }
        match &signature.expansion {
          MacroExpansionType::Expr(semantic) | MacroExpansionType::Definition(semantic) => semantic.validate_applied_type_args(),
          MacroExpansionType::Dynamic | MacroExpansionType::Declarations => Ok(()),
        }
      }
      Self::Syntax(contract) => match contract.as_ref() {
        MacroSyntaxType::Expr(semantic) => semantic.validate_applied_type_args(),
        MacroSyntaxType::Syntax | MacroSyntaxType::SyntaxSymbol | MacroSyntaxType::SyntaxList => Ok(()),
      },
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
      Self::TypeRef(name, args) => {
        for arg in args.iter() {
          arg.validate_applied_type_args()?;
        }

        let resolution_key = type_ref_arity_resolution_key(name);
        let entered = TYPE_REF_ARITY_RESOLUTION_STACK.with(|active| active.borrow_mut().insert(resolution_key.clone()));
        if !entered {
          return Ok(());
        }
        let declared_arity = self
          .resolve_to_struct()
          .map(|definition| ("struct", definition.name.to_string(), definition.generics.len()))
          .or_else(|| {
            self
              .resolve_to_enum()
              .map(|definition| ("enum", definition.name().to_string(), definition.generics().len()))
          });
        TYPE_REF_ARITY_RESOLUTION_STACK.with(|active| {
          active.borrow_mut().remove(&resolution_key);
        });
        if let Some((kind, definition, expected)) = declared_arity
          && args.len() != expected
        {
          return Err(format!(
            "{kind} `{definition}` referenced by `{}` expects {expected} type argument(s), but received {}; apply every declared argument or use a context that can infer them",
            name.trim_start_matches('\''),
            args.len()
          ));
        }
        Ok(())
      }
      Self::StructValue(_)
      | Self::EnumValue(_)
      | Self::StructDef(_)
      | Self::EnumDef(_)
      | Self::Trait(_)
      | Self::TraitSet(_)
      | Self::Custom(_) => Ok(()),
      Self::Bool
      | Self::Number
      | Self::String
      | Self::Symbol
      | Self::Tag
      | Self::AnonymousEnum
      | Self::DynFn
      | Self::Buffer
      | Self::F64Buffer
      | Self::CirruQuote
      | Self::Dynamic
      | Self::TypeVar(_)
      | Self::Nil
      | Self::Unit
      | Self::JsObject
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
      // `:any` is a legacy spelling of `:dynamic`. Keep accepting it at input
      // boundaries, but canonicalize immediately so downstream analysis cannot
      // accidentally treat the two spellings as different contracts.
      "any" => Some(Self::Dynamic),
      "bool" => Some(Self::Bool),
      "number" => Some(Self::Number),
      "string" => Some(Self::String),
      "symbol" => Some(Self::Symbol),
      "tag" => Some(Self::Tag),
      "list" => Some(Self::List(DYNAMIC_TYPE.clone())),
      "map" => Some(Self::Map(DYNAMIC_TYPE.clone(), DYNAMIC_TYPE.clone())),
      "set" => Some(Self::Set(DYNAMIC_TYPE.clone())),
      "tuple" | "enum" => Some(Self::AnonymousEnum),
      "fn" => Some(Self::DynFn),
      "ref" => Some(Self::Ref(DYNAMIC_TYPE.clone())),
      "buffer" => Some(Self::Buffer),
      "f64-buffer" => Some(Self::F64Buffer),
      "cirru-quote" => Some(Self::CirruQuote),
      "nil" => Some(Self::Nil),
      "unit" => Some(Self::Unit),
      "js-object" => Some(Self::JsObject),
      _ => None,
    }
  }

  pub(crate) fn builtin_tag_name(&self) -> Option<&'static str> {
    match self {
      Self::Custom(value) if Self::custom_keyword_matches(value, "any") => Some("dynamic"),
      Self::Bool => Some("bool"),
      Self::Number => Some("number"),
      Self::String => Some("string"),
      Self::Symbol => Some("symbol"),
      Self::Tag => Some("tag"),
      Self::List(_) => Some("list"),
      Self::Map(_, _) => Some("map"),
      Self::DynFn => Some("fn"),
      Self::Set(_) => Some("set"),
      Self::AnonymousEnum => Some("enum"),
      Self::Ref(_) => Some("ref"),
      Self::Buffer => Some("buffer"),
      Self::F64Buffer => Some("f64-buffer"),
      Self::CirruQuote => Some("cirru-quote"),
      Self::Nil => Some("nil"),
      Self::Unit => Some("unit"),
      Self::JsObject => Some("js-object"),
      _ => None,
    }
  }

  /// Canonical schema spelling for built-in types. Snapshot EDN renders symbols
  /// as quoted Cirru forms, so `String` becomes source-level `'String`.
  ///
  /// Lowercase tags remain accepted at every parsing boundary for compatibility,
  /// but writers use these nominal-looking symbols to keep type syntax distinct
  /// from ordinary keyword/tag data.
  pub fn canonical_type_symbol_name(name: &str) -> Option<&'static str> {
    match name.trim_start_matches(':') {
      "any" | "dynamic" | "Dynamic" => Some("Dynamic"),
      "nil" | "Nil" => Some("Nil"),
      "unit" | "Unit" => Some("Unit"),
      "bool" | "Bool" => Some("Bool"),
      "number" | "Number" => Some("Number"),
      "string" | "String" => Some("String"),
      "symbol" | "Symbol" => Some("Symbol"),
      "tag" | "Tag" => Some("Tag"),
      "list" | "List" => Some("List"),
      "map" | "Map" => Some("Map"),
      "set" | "Set" => Some("Set"),
      "fn" | "Fn" => Some("Fn"),
      "macro" | "Macro" => Some("Macro"),
      // Record/Tuple are accepted only as legacy input spellings. Writers and
      // diagnostics always use the Struct/Enum value model.
      "tuple" | "Tuple" | "enum" | "Enum" => Some("Enum"),
      "ref" | "Ref" => Some("Ref"),
      "buffer" | "Buffer" => Some("Buffer"),
      "f64-buffer" | "F64Buffer" => Some("F64Buffer"),
      "cirru-quote" | "CirruQuote" => Some("CirruQuote"),
      "js-object" | "JsObject" => Some("JsObject"),
      "optional" | "Optional" => Some("Optional"),
      "js-nullish" | "JsNullish" => Some("JsNullish"),
      "&" | "variadic" | "Variadic" => Some("Variadic"),
      "record" | "Record" | "struct" | "Struct" => Some("Struct"),
      "struct-def" | "StructDef" => Some("StructDef"),
      "enum-def" | "EnumDef" => Some("EnumDef"),
      "trait" | "Trait" => Some("Trait"),
      "impl" | "Impl" => Some("Impl"),
      _ => None,
    }
  }

  fn builtin_type_from_symbol_name(name: &str) -> Option<Self> {
    match Self::canonical_type_symbol_name(name)? {
      "Dynamic" => Some(Self::Dynamic),
      "Nil" => Some(Self::Nil),
      "Unit" => Some(Self::Unit),
      "Bool" => Some(Self::Bool),
      "Number" => Some(Self::Number),
      "String" => Some(Self::String),
      "Symbol" => Some(Self::Symbol),
      "Tag" => Some(Self::Tag),
      "List" => Some(Self::List(DYNAMIC_TYPE.clone())),
      "Map" => Some(Self::Map(DYNAMIC_TYPE.clone(), DYNAMIC_TYPE.clone())),
      "Set" => Some(Self::Set(DYNAMIC_TYPE.clone())),
      "Fn" | "Macro" => Some(Self::DynFn),
      "Enum" => Some(Self::AnonymousEnum),
      "Ref" => Some(Self::Ref(DYNAMIC_TYPE.clone())),
      "Buffer" => Some(Self::Buffer),
      "F64Buffer" => Some(Self::F64Buffer),
      "CirruQuote" => Some(Self::CirruQuote),
      "JsObject" => Some(Self::JsObject),
      "Struct" => Some(Self::Custom(Arc::new(Calcit::tag("struct")))),
      "StructDef" => Some(Self::Custom(Arc::new(Calcit::tag("struct-def")))),
      "EnumDef" => Some(Self::Custom(Arc::new(Calcit::tag("enum-def")))),
      "Trait" => Some(Self::Custom(Arc::new(Calcit::tag("trait")))),
      "Impl" => Some(Self::Custom(Arc::new(Calcit::tag("impl")))),
      // These forms require a payload and are handled by the type-expression parser.
      "Optional" | "JsNullish" | "Variadic" => None,
      _ => None,
    }
  }

  fn canonical_type_form_name(form: &Calcit) -> Option<&'static str> {
    match form {
      Calcit::Tag(tag) => Self::canonical_type_symbol_name(tag.ref_str()),
      Calcit::Symbol { sym, .. } => Self::canonical_type_symbol_name(sym),
      _ => Self::parse_type_var_form(form).and_then(|name| Self::canonical_type_symbol_name(&name)),
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
    match form {
      Calcit::Symbol { sym, info, .. } => {
        let normalized = Self::normalize_type_ref_name(sym);
        if normalized.contains('/') {
          Some(normalized)
        } else if lookup_def_code_registered(&info.at_ns, &normalized).is_some() {
          Some(Arc::from(format!("{}/{}", info.at_ns, normalized)))
        } else if lookup_def_code_registered(CORE_NS, &normalized).is_some() {
          Some(Arc::from(format!("{CORE_NS}/{normalized}")))
        } else {
          Some(normalized)
        }
      }
      Calcit::Import(import) => Some(Arc::from(format!("{}/{}", import.ns, import.def))),
      _ => Self::parse_type_var_form(form),
    }
  }

  fn type_ref_name_matches(name: &str, target: &str) -> bool {
    let left = name.trim_start_matches('\'').trim_start_matches(':');
    let right = target.trim_start_matches(':');
    left == right || left.rsplit('/').next().is_some_and(|segment| segment == right)
  }

  fn type_ref_resolves_to_struct_name(name: &str, target: &str) -> bool {
    Self::TypeRef(Arc::from(name), Arc::new(vec![]))
      .resolve_to_struct()
      .is_some_and(|resolved| resolved.name.ref_str() == target)
  }

  fn type_ref_resolves_to_enum_name(name: &str, target: &str) -> bool {
    Self::TypeRef(Arc::from(name), Arc::new(vec![]))
      .resolve_to_enum()
      .is_some_and(|resolved| resolved.name().ref_str() == target)
  }

  fn struct_nominal_match(actual: &Arc<CalcitStructDef>, expected: &Arc<CalcitStructDef>) -> Option<bool> {
    if Arc::ptr_eq(actual, expected) {
      return Some(true);
    }
    if actual.definition_ref.is_some() && expected.definition_ref.is_some() {
      Some(actual.same_nominal_definition(expected))
    } else {
      None
    }
  }

  fn enum_nominal_match(actual: &Arc<CalcitEnumDef>, expected: &Arc<CalcitEnumDef>) -> Option<bool> {
    if Arc::ptr_eq(actual, expected) {
      return Some(true);
    }
    if actual.definition_ref().is_some() && expected.definition_ref().is_some() {
      Some(actual.same_nominal_definition(expected))
    } else {
      None
    }
  }

  fn type_ref_matches_struct_definition(name: &str, expected: &Arc<CalcitStructDef>) -> Option<bool> {
    let normalized = name.trim_start_matches('\'').trim_start_matches(':');
    let resolved = Self::TypeRef(Arc::from(normalized), Arc::new(vec![])).resolve_to_struct();
    if let Some(actual) = resolved.map(Arc::new)
      && let Some(matches) = Self::struct_nominal_match(&actual, expected)
    {
      return Some(matches);
    }
    expected
      .definition_ref
      .as_deref()
      .filter(|_| normalized.contains('/'))
      .map(|definition_ref| definition_ref == normalized)
  }

  fn type_ref_matches_enum_definition(name: &str, expected: &Arc<CalcitEnumDef>) -> Option<bool> {
    let normalized = name.trim_start_matches('\'').trim_start_matches(':');
    let resolved = Self::TypeRef(Arc::from(normalized), Arc::new(vec![])).resolve_to_enum();
    if let Some(actual) = resolved.map(Arc::new)
      && let Some(matches) = Self::enum_nominal_match(&actual, expected)
    {
      return Some(matches);
    }
    expected
      .definition_ref()
      .map(AsRef::as_ref)
      .filter(|_| normalized.contains('/'))
      .map(|definition_ref| definition_ref == normalized)
  }

  fn type_ref_nominal_match(actual: &str, expected: &str) -> Option<bool> {
    let actual = actual.trim_start_matches('\'').trim_start_matches(':');
    let expected = expected.trim_start_matches('\'').trim_start_matches(':');
    let actual_ref = Self::TypeRef(Arc::from(actual), Arc::new(vec![]));
    let expected_ref = Self::TypeRef(Arc::from(expected), Arc::new(vec![]));

    if let (Some(actual_def), Some(expected_def)) = (
      actual_ref.resolve_to_struct().map(Arc::new),
      expected_ref.resolve_to_struct().map(Arc::new),
    ) && let Some(matches) = Self::struct_nominal_match(&actual_def, &expected_def)
    {
      return Some(matches);
    }
    if let (Some(actual_def), Some(expected_def)) = (
      actual_ref.resolve_to_enum().map(Arc::new),
      expected_ref.resolve_to_enum().map(Arc::new),
    ) && let Some(matches) = Self::enum_nominal_match(&actual_def, &expected_def)
    {
      return Some(matches);
    }
    if actual.contains('/') && expected.contains('/') {
      return Some(actual == expected);
    }
    None
  }

  fn type_ref_matches_trait(name: &str, target: &CalcitTrait) -> bool {
    let stripped = name.trim_start_matches('\'').trim_start_matches(':');
    if target
      .definition_ref
      .as_deref()
      .is_some_and(|definition_ref| definition_ref == stripped)
    {
      return true;
    }
    if resolve_type_ref_as_schema(stripped).is_some_and(
      |resolved| matches!(resolved.as_ref(), Self::Trait(resolved_trait) if Self::trait_references_match(resolved_trait, target)),
    ) {
      return true;
    }
    target.definition_ref.is_none() && target.runtime_id.is_none() && Self::type_ref_name_matches(stripped, target.name.ref_str())
  }

  /// Match an actual trait reference against an expected one without letting
  /// an unqualified placeholder erase runtime identity in the reverse direction.
  fn trait_references_match(actual: &CalcitTrait, expected: &CalcitTrait) -> bool {
    actual.matches_reference(expected)
      || (actual.definition_ref.is_some() && expected.definition_ref.is_some() && expected.matches_reference(actual))
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
        "features" => {
          fields.has_any = true;
          if fields.features.is_none() {
            fields.features = Some(value);
          }
        }
        "async" => {
          fields.has_any = true;
          if fields.async_invocation.is_none() {
            fields.async_invocation = Some(value);
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
      if let Some(value) = Self::extract_schema_value_single(item, "generics")
        && let Some(vars) = Self::parse_generics_list(value)
      {
        return Some(vars);
      }
    }
    None
  }

  /// Extract the element type declared by `:rest` from a schema hint-fn form.
  pub fn extract_rest_type_from_hint_form(form: &Calcit) -> Option<Arc<CalcitTypeAnnotation>> {
    let generics = Self::extract_generics_from_hint_form(form).unwrap_or_default();
    let items = Self::get_hint_fn_items(form)?;
    for item in items.iter().skip(1) {
      if let Some(type_expr) = Self::extract_schema_value_single(item, "rest") {
        return Some(CalcitTypeAnnotation::parse_type_annotation_form_with_generics(
          type_expr,
          generics.as_slice(),
        ));
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

  fn trait_bound_with_source_ref(form: &Calcit, trait_def: &Arc<CalcitTrait>) -> Arc<CalcitTrait> {
    if trait_def.definition_ref.is_some() {
      return trait_def.clone();
    }
    let Some(source_name) = Self::extract_type_ref_name(form) else {
      return trait_def.clone();
    };
    let Some((ns, def)) = source_name.rsplit_once('/') else {
      return trait_def.clone();
    };
    if def != trait_def.name.ref_str() {
      return trait_def.clone();
    }
    Arc::new(trait_def.as_ref().clone().with_definition_ref(ns, def))
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
          CalcitTypeAnnotation::Trait(trait_def) => traits.push(Self::trait_bound_with_source_ref(item, trait_def)),
          CalcitTypeAnnotation::TypeRef(name, args) if strict_named_refs && args.is_empty() => {
            traits.push(Arc::new(CalcitTrait::new_reference(name)))
          }
          _ => return None,
        }
      }
      if !traits.is_empty() {
        return Some(traits);
      }
    }

    match Self::parse_type_annotation_form_inner(form, generics, strict_named_refs).as_ref() {
      CalcitTypeAnnotation::Trait(trait_def) => Some(vec![Self::trait_bound_with_source_ref(form, trait_def)]),
      CalcitTypeAnnotation::TypeRef(name, args) if strict_named_refs && args.is_empty() => {
        Some(vec![Arc::new(CalcitTrait::new_reference(name))])
      }
      _ => None,
    }
  }

  pub(crate) fn parse_where_bounds_form(form: &Calcit, generics: &[Arc<str>], strict_named_refs: bool) -> Vec<CalcitGenericBound> {
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
        Edn::Symbol(sym) if !Self::generics_contains(generics, sym.as_ref()) => {
          Some(Arc::new(CalcitTrait::new_reference(sym.trim_start_matches('\''))))
        }
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

  fn parse_fn_features_from_form(form: Option<&Calcit>) -> Arc<HashSet<EdnTag>> {
    let Some(form) = form else {
      return Arc::new(HashSet::new());
    };
    match form {
      Calcit::Set(xs) => {
        let mut features = HashSet::with_capacity(xs.size());
        for item in xs.iter() {
          if let Calcit::Tag(tag) = item
            && tag.ref_str() != ASYNC_INVOCATION_FEATURE
          {
            features.insert(tag.clone());
          }
        }
        Arc::new(features)
      }
      _ => Arc::new(HashSet::new()),
    }
  }

  /// Return whether a `hint-fn` schema marks the surrounding function as async.
  /// This is the checked invocation contract shared with JavaScript lowering.
  pub(crate) fn hint_form_marks_async(form: &Calcit) -> bool {
    let Some(items) = Self::get_hint_fn_items(form) else {
      return false;
    };
    items
      .get(1)
      .and_then(|item| Self::extract_schema_value_single(item, "async"))
      .is_some_and(|value| !matches!(value, Calcit::Nil | Calcit::Unit | Calcit::Bool(false)))
  }

  /// Return whether a source or preprocessed function definition carries an async hint.
  pub(crate) fn function_form_marks_async(form: &Calcit) -> bool {
    let Calcit::List(items) = form else {
      return false;
    };
    items.iter().skip(3).any(Self::hint_form_marks_async)
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

    let mut features = Self::parse_fn_features_from_form(fields.features).as_ref().clone();
    if fields
      .async_invocation
      .is_some_and(|value| !matches!(value, Calcit::Nil | Calcit::Unit | Calcit::Bool(false)))
    {
      features.insert(EdnTag::from(ASYNC_INVOCATION_FEATURE));
    }
    let features = Arc::new(features);

    Some(Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(local_generics),
      where_bounds: Arc::new(where_bounds),
      arg_types,
      return_type,
      fn_kind,
      rest_type,
      features,
    }))))
  }

  /// Extract a complete function annotation from the schema map carried by `hint-fn`.
  ///
  /// Both supported forms are accepted:
  ///
  /// - `(hint-fn schema)` inside a function body;
  /// - `(hint-fn target schema)` for refining a local function binding.
  pub fn extract_fn_annotation_from_hint_form(form: &Calcit) -> Option<Arc<CalcitTypeAnnotation>> {
    let items = Self::get_hint_fn_items(form)?;
    for item in items.iter().skip(1) {
      if let Some(annotation) = Self::parse_fn_annotation_from_schema_form(item, &[], true) {
        return Some(annotation);
      }
    }
    None
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
      Edn::Enum(view) => Calcit::Enum(CalcitEnumValue {
        tag: Arc::new(Self::edn_type_to_calcit(&Edn::Symbol(view.variant.clone()))),
        extra: view.extra.iter().map(Self::edn_type_to_calcit).collect(),
        sum_type: None,
      }),
      _ => Calcit::Nil,
    }
  }

  /// Parse a standalone schema type expression from its snapshot EDN form.
  ///
  /// Unlike function schemas, top-level data annotations such as
  /// `(:: :ref :number)` or `(:: :list 'app/Item)` do not carry an argument
  /// scope. Named symbols are therefore interpreted as type references.
  pub fn parse_type_annotation_from_edn(form: &Edn) -> Arc<CalcitTypeAnnotation> {
    Self::parse_type_annotation_form_inner(&Self::edn_type_to_calcit(form), &[], true)
  }

  /// Parse a phase-aware macro signature. Runtime function schemas are not
  /// macro contracts and must be rejected by the Snapshot loader.
  pub fn parse_macro_signature_from_edn(schema: &Edn) -> Option<MacroSignature> {
    let (map, wrapped_macro) = match schema {
      Edn::Map(map) => (map, false),
      Edn::Enum(view) if matches!(view.variant.as_ref(), "macro" | "Macro") => match view.extra.first() {
        Some(Edn::Map(map)) => (map, true),
        _ => return None,
      },
      _ => return None,
    };
    let tagged_macro = matches!(map.tag_get("kind"), Some(Edn::Tag(tag)) if tag.ref_str() == "macro");
    if !wrapped_macro && !tagged_macro {
      return None;
    }

    let strict = ["required", "optional", "expansion", "capabilities"]
      .iter()
      .any(|key| map.tag_get(key).is_some());
    if !strict {
      return None;
    }

    let generics: Vec<Arc<str>> = match map.tag_get("generics") {
      None => vec![],
      Some(Edn::List(xs)) => xs
        .0
        .iter()
        .map(|item| match item {
          Edn::Symbol(name) if !name.starts_with('\'') => Some(name.clone()),
          _ => None,
        })
        .collect::<Option<Vec<_>>>()?,
      Some(_) => return None,
    };
    let parse_contracts = |field: Option<&Edn>| -> Option<Vec<MacroSyntaxType>> {
      match field {
        None => Some(vec![]),
        Some(Edn::List(xs)) => xs
          .0
          .iter()
          .map(|item| MacroSignature::parse_contract(item, generics.as_slice()))
          .collect(),
        Some(_) => None,
      }
    };
    let required_inputs = parse_contracts(map.tag_get("required"))?;
    let optional_inputs = parse_contracts(map.tag_get("optional"))?;
    let rest_input = match map.tag_get("rest") {
      Some(item) => Some(MacroSignature::parse_contract(item, generics.as_slice())?),
      None => None,
    };
    let expansion = MacroSignature::parse_expansion(map.tag_get("expansion"), generics.as_slice())?;
    let where_bounds = map
      .tag_get("where")
      .and_then(|value| Self::parse_where_bounds_from_edn(value, generics.as_slice()))
      .unwrap_or_default();
    let features = map
      .tag_get("features")
      .and_then(|value| match value {
        Edn::Set(xs) => Some(Arc::new(
          xs.0
            .iter()
            .filter_map(|item| match item {
              Edn::Tag(tag) => Some(tag.clone()),
              _ => None,
            })
            .collect(),
        )),
        _ => None,
      })
      .unwrap_or_default();
    let capabilities = match map.tag_get("capabilities") {
      None => Arc::new(HashSet::new()),
      Some(Edn::Set(xs)) => Arc::new(
        xs.0
          .iter()
          .map(|item| match item {
            Edn::Tag(tag) => MacroCapability::parse(tag.ref_str()),
            _ => None,
          })
          .collect::<Option<HashSet<_>>>()?,
      ),
      Some(_) => return None,
    };
    Some(MacroSignature {
      generics: Arc::new(generics),
      where_bounds: Arc::new(where_bounds),
      required_inputs: Arc::new(required_inputs),
      optional_inputs: Arc::new(optional_inputs),
      rest_input,
      expansion,
      capabilities,
      features,
    })
  }

  /// Parse a schema [`Edn`] map value (as stored in [`crate::snapshot::CodeEntry::schema`])
  /// directly into a [`CalcitFnTypeAnnotation`], without going through a Cirru/Calcit roundtrip.
  ///
  /// Accepts both a plain schema map and the canonical wrapped form `(:: 'Fn ({} ...))`
  /// or `(:: 'Macro ({} ...))`, plus legacy `:fn` / `:macro` tags.
  /// Returns `None` only when the input does not look like a function schema at all.
  pub fn parse_fn_schema_from_edn(schema: &Edn) -> Option<CalcitFnTypeAnnotation> {
    let mut wrapped_kind: Option<SchemaKind> = None;
    let map = match schema {
      Edn::Map(map) => map,
      Edn::Enum(view) if matches!(view.variant.as_ref(), "fn" | "macro" | "Fn" | "Macro") => {
        wrapped_kind = match view.variant.as_ref() {
          "macro" | "Macro" => Some(SchemaKind::Macro),
          _ => Some(SchemaKind::Fn),
        };
        match view.extra.first() {
          Some(Edn::Map(map)) => map,
          _ => return None,
        }
      }
      _ => return None,
    };

    let has_schema_fields = ["kind", "args", "return", "generics", "where", "rest", "features", "async"]
      .iter()
      .any(|key| map.tag_get(key).is_some());
    if !has_schema_fields {
      return None;
    }
    if ["required", "optional", "expansion", "capabilities"]
      .iter()
      .any(|key| map.tag_get(key).is_some())
    {
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
    let mut features = map
      .tag_get("features")
      .and_then(|v| {
        if let Edn::Set(xs) = v {
          let mut set = HashSet::with_capacity(xs.len());
          for item in xs.0.iter() {
            if let Edn::Tag(tag) = item {
              set.insert(tag.clone());
            }
          }
          Some(Arc::new(set))
        } else {
          None
        }
      })
      .unwrap_or_default()
      .as_ref()
      .clone();
    features.retain(|feature| feature.ref_str() != ASYNC_INVOCATION_FEATURE);
    if matches!(map.tag_get("async"), Some(Edn::Bool(true))) {
      features.insert(EdnTag::from(ASYNC_INVOCATION_FEATURE));
    }
    let features = Arc::new(features);
    Some(CalcitFnTypeAnnotation {
      generics: Arc::new(generics),
      where_bounds: Arc::new(where_bounds),
      arg_types,
      return_type,
      fn_kind,
      rest_type,
      features,
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

  /// Summarize definition code for `calcit query def` output.
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
            if is_assert
              && inner.len() == 3
              && let (Some(Calcit::Symbol { sym, .. }), Some(type_form)) = (inner.get(1), inner.get(2))
            {
              let t = Self::parse_type_annotation_form_with_generics(type_form, generics.as_slice());
              arg_types.insert(sym.to_owned(), t);
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
        features: Arc::new(HashSet::new()),
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

    // A definition value used in a type-expression position denotes its
    // instance type. Runtime expression inference still classifies the same
    // value as StructDef/EnumDef; this branch is deliberately schema-only.
    match form {
      Calcit::StructDef(struct_def) => {
        return Arc::new(CalcitTypeAnnotation::Struct(Arc::new(struct_def.to_owned()), Arc::new(vec![])));
      }
      Calcit::EnumDef(enum_def) => {
        return Arc::new(CalcitTypeAnnotation::Enum(Arc::new(enum_def.to_owned()), Arc::new(vec![])));
      }
      _ => {}
    }

    if let Some(name) = Self::parse_type_var_form(form) {
      if let Some(builtin) = Self::builtin_type_from_symbol_name(&name) {
        return Arc::new(builtin);
      }
      // A qualified quoted name cannot be a lexical type variable. Preserve
      // it as a nominal reference even in unscoped `assert-type` forms, so a
      // later Struct field access can resolve its declaration.
      return if (strict_named_refs || name.contains('/')) && !Self::generics_contains(generics, &name) {
        let qualified_name = Self::extract_type_ref_name(form).unwrap_or_else(|| name.clone());
        Arc::new(CalcitTypeAnnotation::TypeRef(qualified_name, Arc::new(vec![])))
      } else {
        Arc::new(CalcitTypeAnnotation::TypeVar(name))
      };
    }

    if let Calcit::Symbol { sym, .. } = form {
      if let Some(builtin) = Self::builtin_type_from_symbol_name(sym) {
        return Arc::new(builtin);
      }
      if sym.starts_with('\'') {
        let stripped = sym.trim_start_matches('\'');
        let n_quotes = sym.len() - stripped.len();
        if n_quotes > 1 {
          eprintln!("[Error] Type variable `{sym}` has excess leading quotes — expected a single-quoted uppercase symbol like `'T`");
        }
        return if (strict_named_refs || stripped.contains('/')) && !Self::generics_contains(generics, stripped) {
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

    if let Calcit::Enum(enum_value) = form {
      if let Some(struct_def) = resolve_struct_def(enum_value.tag.as_ref()) {
        let args = enum_value.extra.iter().map(parse_nested).collect::<Vec<_>>();
        return Arc::new(CalcitTypeAnnotation::Struct(Arc::new(struct_def), Arc::new(args)));
      }
      if let Calcit::Tag(tag) = enum_value.tag.as_ref() {
        if is_optional_tag(tag) {
          if enum_value.extra.len() != 1 {
            eprintln!("[Warn] :optional expects 1 argument, got {}", enum_value.extra.len());
          }
          if let Some(inner_form) = enum_value.extra.first() {
            return Arc::new(CalcitTypeAnnotation::Optional(parse_nested(inner_form)));
          }
        }
        if is_list_tag(tag) {
          if enum_value.extra.len() > 1 {
            eprintln!("[Warn] :list expects at most 1 argument, got {}", enum_value.extra.len());
          }
          if let Some(inner_form) = enum_value.extra.first() {
            return Arc::new(CalcitTypeAnnotation::List(parse_nested(inner_form)));
          }
          return Arc::new(CalcitTypeAnnotation::List(DYNAMIC_TYPE.clone()));
        }
        if is_map_tag(tag) {
          if enum_value.extra.len() > 2 {
            eprintln!("[Warn] :map expects at most 2 arguments, got {}", enum_value.extra.len());
          }
          let key_type = enum_value.extra.first().map(parse_nested).unwrap_or_else(|| DYNAMIC_TYPE.clone());
          let val_type = enum_value.extra.get(1).map(parse_nested).unwrap_or_else(|| DYNAMIC_TYPE.clone());
          return Arc::new(CalcitTypeAnnotation::Map(key_type, val_type));
        }
        if is_set_tag(tag) {
          if enum_value.extra.len() > 1 {
            eprintln!("[Warn] :set expects at most 1 argument, got {}", enum_value.extra.len());
          }
          if let Some(inner_form) = enum_value.extra.first() {
            return Arc::new(CalcitTypeAnnotation::Set(parse_nested(inner_form)));
          }
          return Arc::new(CalcitTypeAnnotation::Set(DYNAMIC_TYPE.clone()));
        }
        if is_ref_tag(tag) {
          if enum_value.extra.len() > 1 {
            eprintln!("[Warn] :ref expects at most 1 argument, got {}", enum_value.extra.len());
          }
          if let Some(inner_form) = enum_value.extra.first() {
            return Arc::new(CalcitTypeAnnotation::Ref(parse_nested(inner_form)));
          }
          return Arc::new(CalcitTypeAnnotation::Ref(DYNAMIC_TYPE.clone()));
        }
        if tag.ref_str().trim_start_matches(':') == "fn" {
          if let Some(schema_form) = enum_value.extra.first()
            && let Some(parsed) = Self::parse_fn_annotation_from_schema_form(schema_form, generics, strict_named_refs)
          {
            return parsed;
          }
          if enum_value.extra.is_empty() {
            return Arc::new(CalcitTypeAnnotation::DynFn);
          }
          emit_legacy_fn_type_syntax_warning(":: :fn $ {} ...", form);
          return Arc::new(CalcitTypeAnnotation::DynFn);
        }
      }

      let base_name = Self::canonical_type_form_name(enum_value.tag.as_ref());
      let base = Self::parse_type_annotation_form_inner(enum_value.tag.as_ref(), generics, strict_named_refs);
      let args = enum_value.extra.iter().map(parse_nested).collect::<Vec<_>>();
      if let Some(name) = base_name {
        if args.is_empty()
          && matches!(
            name,
            "Dynamic"
              | "Nil"
              | "Unit"
              | "Bool"
              | "Number"
              | "String"
              | "Symbol"
              | "Tag"
              | "List"
              | "Map"
              | "Set"
              | "Fn"
              | "Tuple"
              | "Ref"
              | "Buffer"
              | "CirruQuote"
              | "JsObject"
              | "Record"
              | "Struct"
              | "Enum"
              | "Trait"
              | "Impl"
          )
        {
          return base;
        }
        match name {
          "Optional" if args.len() == 1 => return Arc::new(CalcitTypeAnnotation::Optional(args[0].clone())),
          "JsNullish" if args.len() == 1 => return Arc::new(CalcitTypeAnnotation::JsNullish(args[0].clone())),
          "Variadic" if args.len() == 1 => return Arc::new(CalcitTypeAnnotation::Variadic(args[0].clone())),
          "List" => {
            return Arc::new(CalcitTypeAnnotation::List(
              args.first().cloned().unwrap_or_else(|| DYNAMIC_TYPE.clone()),
            ));
          }
          "Map" => {
            return Arc::new(CalcitTypeAnnotation::Map(
              args.first().cloned().unwrap_or_else(|| DYNAMIC_TYPE.clone()),
              args.get(1).cloned().unwrap_or_else(|| DYNAMIC_TYPE.clone()),
            ));
          }
          "Set" => {
            return Arc::new(CalcitTypeAnnotation::Set(
              args.first().cloned().unwrap_or_else(|| DYNAMIC_TYPE.clone()),
            ));
          }
          "Ref" => {
            return Arc::new(CalcitTypeAnnotation::Ref(
              args.first().cloned().unwrap_or_else(|| DYNAMIC_TYPE.clone()),
            ));
          }
          "Fn" => {
            if let Some(parsed) = enum_value
              .extra
              .first()
              .and_then(|schema_form| Self::parse_fn_annotation_from_schema_form(schema_form, generics, strict_named_refs))
            {
              return parsed;
            }
          }
          _ => {}
        }
      }
      match base.as_ref() {
        CalcitTypeAnnotation::Struct(struct_def, _) => {
          return Arc::new(CalcitTypeAnnotation::Struct(struct_def.clone(), Arc::new(args)));
        }
        CalcitTypeAnnotation::Enum(enum_def, _) => {
          return Arc::new(CalcitTypeAnnotation::Enum(enum_def.clone(), Arc::new(args)));
        }
        CalcitTypeAnnotation::TypeRef(name, _) if strict_named_refs || name.contains('/') => {
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
          return Arc::new(CalcitTypeAnnotation::List(DYNAMIC_TYPE.clone()));
        }
        if is_map_tag(tag) {
          if xs.len() > 3 {
            eprintln!("[Warn] :map expects at most 2 arguments, got {}", xs.len() as i64 - 1);
          }
          let key_type = xs.get(1).map(parse_nested).unwrap_or_else(|| DYNAMIC_TYPE.clone());
          let val_type = xs.get(2).map(parse_nested).unwrap_or_else(|| DYNAMIC_TYPE.clone());
          return Arc::new(CalcitTypeAnnotation::Map(key_type, val_type));
        }
        if is_set_tag(tag) {
          if xs.len() > 2 {
            eprintln!("[Warn] :set expects at most 1 argument, got {}", xs.len() as i64 - 1);
          }
          if let Some(inner_form) = xs.get(1) {
            return Arc::new(CalcitTypeAnnotation::Set(parse_nested(inner_form)));
          }
          return Arc::new(CalcitTypeAnnotation::Set(DYNAMIC_TYPE.clone()));
        }
        if is_ref_tag(tag) {
          if xs.len() > 2 {
            eprintln!("[Warn] :ref expects at most 1 argument, got {}", xs.len() as i64 - 1);
          }
          if let Some(inner_form) = xs.get(1) {
            return Arc::new(CalcitTypeAnnotation::Ref(parse_nested(inner_form)));
          }
          return Arc::new(CalcitTypeAnnotation::Ref(DYNAMIC_TYPE.clone()));
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

      let is_enum_constructor = match xs.first() {
        Some(Calcit::Proc(CalcitProc::NativeEnum)) => true,
        Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "::" => true,
        _ => false,
      };

      if is_enum_constructor {
        if xs.len() == 3
          && let (Some(Calcit::Tag(marker)), Some(inner_form)) = (xs.get(1), xs.get(2))
          && marker.ref_str().trim_start_matches(':') == "&"
        {
          return Arc::new(CalcitTypeAnnotation::Variadic(parse_nested(inner_form)));
        }

        if let Some(Calcit::Tag(tag)) = xs.get(1)
          && is_optional_tag(tag)
        {
          if xs.len() != 3 {
            eprintln!("[Warn] :optional expects 1 argument, got {}", xs.len() as i64 - 2);
          }
          if let Some(inner_form) = xs.get(2) {
            return Arc::new(CalcitTypeAnnotation::Optional(parse_nested(inner_form)));
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
            return Arc::new(CalcitTypeAnnotation::List(DYNAMIC_TYPE.clone()));
          }
          if tag_name == "map" {
            let key_type = xs.get(2).map(parse_nested).unwrap_or_else(|| DYNAMIC_TYPE.clone());
            let val_type = xs.get(3).map(parse_nested).unwrap_or_else(|| DYNAMIC_TYPE.clone());
            return Arc::new(CalcitTypeAnnotation::Map(key_type, val_type));
          }
          if tag_name == "set" {
            if let Some(inner_form) = xs.get(2) {
              return Arc::new(CalcitTypeAnnotation::Set(parse_nested(inner_form)));
            }
            return Arc::new(CalcitTypeAnnotation::Set(DYNAMIC_TYPE.clone()));
          }
          if tag_name == "ref" {
            if let Some(inner_form) = xs.get(2) {
              return Arc::new(CalcitTypeAnnotation::Ref(parse_nested(inner_form)));
            }
            return Arc::new(CalcitTypeAnnotation::Ref(DYNAMIC_TYPE.clone()));
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
          let base_name = Self::canonical_type_form_name(base_form);
          let base = Self::parse_type_annotation_form_inner(base_form, generics, strict_named_refs);
          let args = xs
            .iter()
            .skip(2)
            .map(|item| Self::parse_type_annotation_form_inner(item, generics, strict_named_refs))
            .collect::<Vec<_>>();
          if let Some(name) = base_name {
            if args.is_empty()
              && matches!(
                name,
                "Dynamic"
                  | "Nil"
                  | "Unit"
                  | "Bool"
                  | "Number"
                  | "String"
                  | "Symbol"
                  | "Tag"
                  | "List"
                  | "Map"
                  | "Set"
                  | "Fn"
                  | "Tuple"
                  | "Ref"
                  | "Buffer"
                  | "CirruQuote"
                  | "JsObject"
                  | "Record"
                  | "Struct"
                  | "Enum"
                  | "Trait"
                  | "Impl"
              )
            {
              return base;
            }
            match name {
              "Optional" if args.len() == 1 => return Arc::new(CalcitTypeAnnotation::Optional(args[0].clone())),
              "JsNullish" if args.len() == 1 => return Arc::new(CalcitTypeAnnotation::JsNullish(args[0].clone())),
              "Variadic" if args.len() == 1 => return Arc::new(CalcitTypeAnnotation::Variadic(args[0].clone())),
              "List" => {
                return Arc::new(CalcitTypeAnnotation::List(
                  args.first().cloned().unwrap_or_else(|| DYNAMIC_TYPE.clone()),
                ));
              }
              "Map" => {
                return Arc::new(CalcitTypeAnnotation::Map(
                  args.first().cloned().unwrap_or_else(|| DYNAMIC_TYPE.clone()),
                  args.get(1).cloned().unwrap_or_else(|| DYNAMIC_TYPE.clone()),
                ));
              }
              "Set" => {
                return Arc::new(CalcitTypeAnnotation::Set(
                  args.first().cloned().unwrap_or_else(|| DYNAMIC_TYPE.clone()),
                ));
              }
              "Ref" => {
                return Arc::new(CalcitTypeAnnotation::Ref(
                  args.first().cloned().unwrap_or_else(|| DYNAMIC_TYPE.clone()),
                ));
              }
              "Fn" => {
                if let Some(parsed) = xs
                  .get(2)
                  .and_then(|schema_form| Self::parse_fn_annotation_from_schema_form(schema_form, generics, strict_named_refs))
                {
                  return parsed;
                }
              }
              _ => {}
            }
          }
          match base.as_ref() {
            CalcitTypeAnnotation::Struct(struct_def, _) => {
              return Arc::new(CalcitTypeAnnotation::Struct(struct_def.clone(), Arc::new(args)));
            }
            CalcitTypeAnnotation::Enum(enum_def, _) => {
              return Arc::new(CalcitTypeAnnotation::Enum(enum_def.clone(), Arc::new(args)));
            }
            CalcitTypeAnnotation::TypeRef(name, _) if strict_named_refs || name.contains('/') => {
              return Arc::new(CalcitTypeAnnotation::TypeRef(name.clone(), Arc::new(args)));
            }
            // In an unscoped assertion, a quoted leaf is historically parsed as a TypeVar.
            // Type variables are not higher-kinded in Calcit, so applying arguments to one is
            // unambiguously a named type application such as `(:: 'Box :number)`.
            CalcitTypeAnnotation::TypeVar(name) if !args.is_empty() => {
              return Arc::new(CalcitTypeAnnotation::TypeRef(name.clone(), Arc::new(args)));
            }
            _ => {}
          }
        }
      }
    }

    // Keep a self-referential field annotation nominal. Resolving it would
    // eagerly rebuild its struct and unfold the declaration until the process
    // exhausts its stack. Other named forms retain their existing resolution
    // behavior (notably trait references).
    if strict_named_refs
      && matches!(form, Calcit::Symbol { sym, info, .. } if sym == &info.at_def)
      && let Some(name) = Self::extract_type_ref_name(form)
    {
      return Arc::new(CalcitTypeAnnotation::TypeRef(name, Arc::new(vec![])));
    }

    if let Some(resolved) = resolve_calcit_value(form) {
      match resolved {
        Calcit::Trait(trait_def) => return Arc::new(CalcitTypeAnnotation::Trait(Arc::new(trait_def))),
        Calcit::StructDef(struct_def) if !strict_named_refs => {
          return Arc::new(CalcitTypeAnnotation::Struct(Arc::new(struct_def), Arc::new(vec![])));
        }
        Calcit::EnumDef(enum_def) if !strict_named_refs => {
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
    let mut remaining = 128;
    self.to_brief_string_bounded(0, &mut remaining)
  }

  fn to_brief_string_bounded(&self, depth: usize, remaining: &mut usize) -> String {
    if depth >= TYPE_DIAGNOSTIC_DEPTH_LIMIT || *remaining == 0 {
      return "…".to_owned();
    }
    *remaining -= 1;
    if let Some(tag) = self.builtin_tag_name() {
      return format!(":{tag}");
    }

    match self {
      Self::Fn(signature) => signature.render_signature_brief_bounded(depth + 1, remaining),
      Self::Variadic(inner) => format!("&{}", inner.to_brief_string_bounded(depth + 1, remaining)),
      Self::List(inner) => format!("list<{}>", inner.to_brief_string_bounded(depth + 1, remaining)),
      Self::Map(k, v) => format!(
        "map<{},{}>",
        k.to_brief_string_bounded(depth + 1, remaining),
        v.to_brief_string_bounded(depth + 1, remaining)
      ),
      Self::Set(inner) => format!("set<{}>", inner.to_brief_string_bounded(depth + 1, remaining)),
      Self::Ref(inner) => format!("ref<{}>", inner.to_brief_string_bounded(depth + 1, remaining)),
      Self::Custom(inner) => format!("{inner}"),
      Self::Optional(inner) => format!("{}?", inner.to_brief_string_bounded(depth + 1, remaining)),
      Self::JsNullish(inner) => format!("js-nullish<{}>", inner.to_brief_string_bounded(depth + 1, remaining)),
      Self::Struct(base, args) => {
        if args.is_empty() {
          format!("struct {}", base.name)
        } else {
          let rendered = render_bounded_type_list(args, depth + 1, remaining, CalcitTypeAnnotation::to_brief_string_bounded);
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
          let rendered = render_bounded_type_list(args, depth + 1, remaining, CalcitTypeAnnotation::to_brief_string_bounded);
          format!("'{name}<{rendered}>")
        }
      }
      Self::Enum(enum_def, args) => {
        if args.is_empty() {
          format!("enum {}", enum_def.name())
        } else {
          let rendered = render_bounded_type_list(args, depth + 1, remaining, CalcitTypeAnnotation::to_brief_string_bounded);
          format!("enum {}<{}>", enum_def.name(), rendered)
        }
      }
      Self::StructDef(struct_def) => format!("struct-def {}", struct_def.name),
      Self::EnumDef(enum_def) => format!("enum-def {}", enum_def.name()),
      Self::StructValue(struct_def) => format!("struct {}", struct_def.name),
      Self::EnumValue(enum_def) => format!("enum {}", enum_def.name()),
      Self::Dynamic => "dynamic".to_string(),
      Self::TypeSlot(name) => format!("type-slot({name})"),
      _ => "unknown".to_string(),
    }
  }

  /// Substitute all `TypeVar` occurrences with their bound types from `bindings`.
  /// Returns a new annotation with variables resolved; unbound variables remain as-is.
  pub fn substitute_type_vars(&self, bindings: &TypeBindings) -> Arc<CalcitTypeAnnotation> {
    enum Unary {
      List,
      Set,
      Ref,
      Optional,
      JsNullish,
      Variadic,
    }

    enum Task<'a> {
      Visit(&'a CalcitTypeAnnotation),
      Cache(usize),
      BuildUnary(Unary),
      BuildMap,
      BuildTypeRef(Arc<str>, usize),
      BuildStruct(Arc<CalcitStructDef>, usize),
      BuildEnum(Arc<CalcitEnumDef>, usize),
      BuildFn(Arc<CalcitFnTypeAnnotation>, usize, bool),
    }

    let mut tasks = vec![Task::Visit(self)];
    let mut values: Vec<Arc<CalcitTypeAnnotation>> = vec![];
    let mut cache: HashMap<usize, Arc<CalcitTypeAnnotation>> = HashMap::new();
    while let Some(task) = tasks.pop() {
      match task {
        Task::Visit(annotation) => {
          let cache_key = std::ptr::from_ref(annotation) as usize;
          if let Some(cached) = cache.get(&cache_key) {
            values.push(cached.clone());
            continue;
          }
          tasks.push(Task::Cache(cache_key));
          match annotation {
            Self::TypeVar(name) => values.push(bindings.get(name).cloned().unwrap_or_else(|| Arc::new(annotation.clone()))),
            Self::TypeRef(name, args) => {
              tasks.push(Task::BuildTypeRef(name.clone(), args.len()));
              tasks.extend(args.iter().rev().map(|arg| Task::Visit(arg)));
            }
            Self::List(inner) => {
              tasks.push(Task::BuildUnary(Unary::List));
              tasks.push(Task::Visit(inner));
            }
            Self::Set(inner) => {
              tasks.push(Task::BuildUnary(Unary::Set));
              tasks.push(Task::Visit(inner));
            }
            Self::Ref(inner) => {
              tasks.push(Task::BuildUnary(Unary::Ref));
              tasks.push(Task::Visit(inner));
            }
            Self::Optional(inner) => {
              tasks.push(Task::BuildUnary(Unary::Optional));
              tasks.push(Task::Visit(inner));
            }
            Self::JsNullish(inner) => {
              tasks.push(Task::BuildUnary(Unary::JsNullish));
              tasks.push(Task::Visit(inner));
            }
            Self::Variadic(inner) => {
              tasks.push(Task::BuildUnary(Unary::Variadic));
              tasks.push(Task::Visit(inner));
            }
            Self::Map(key, value) => {
              tasks.push(Task::BuildMap);
              tasks.push(Task::Visit(value));
              tasks.push(Task::Visit(key));
            }
            Self::Struct(base, args) => {
              tasks.push(Task::BuildStruct(base.clone(), args.len()));
              tasks.extend(args.iter().rev().map(|arg| Task::Visit(arg)));
            }
            Self::Enum(base, args) => {
              tasks.push(Task::BuildEnum(base.clone(), args.len()));
              tasks.extend(args.iter().rev().map(|arg| Task::Visit(arg)));
            }
            Self::Fn(signature) => {
              tasks.push(Task::BuildFn(
                signature.clone(),
                signature.arg_types.len(),
                signature.rest_type.is_some(),
              ));
              if let Some(rest) = &signature.rest_type {
                tasks.push(Task::Visit(rest));
              }
              tasks.push(Task::Visit(&signature.return_type));
              tasks.extend(signature.arg_types.iter().rev().map(|arg| Task::Visit(arg)));
            }
            _ => values.push(Arc::new(annotation.clone())),
          }
        }
        Task::Cache(cache_key) => {
          cache.insert(cache_key, values.last().expect("substitution cached result").clone());
        }
        Task::BuildUnary(kind) => {
          let inner = values.pop().expect("substitution unary result");
          values.push(Arc::new(match kind {
            Unary::List => Self::List(inner),
            Unary::Set => Self::Set(inner),
            Unary::Ref => Self::Ref(inner),
            Unary::Optional => Self::Optional(inner),
            Unary::JsNullish => Self::JsNullish(inner),
            Unary::Variadic => Self::Variadic(inner),
          }));
        }
        Task::BuildMap => {
          let value = values.pop().expect("substitution map value");
          let key = values.pop().expect("substitution map key");
          values.push(Arc::new(Self::Map(key, value)));
        }
        Task::BuildTypeRef(name, count) => {
          let args = values.split_off(values.len() - count);
          values.push(Arc::new(Self::TypeRef(name, Arc::new(args))));
        }
        Task::BuildStruct(base, count) => {
          let args = values.split_off(values.len() - count);
          values.push(Arc::new(Self::Struct(base, Arc::new(args))));
        }
        Task::BuildEnum(base, count) => {
          let args = values.split_off(values.len() - count);
          values.push(Arc::new(Self::Enum(base, Arc::new(args))));
        }
        Task::BuildFn(signature, arg_count, has_rest) => {
          let child_count = arg_count + 1 + usize::from(has_rest);
          let mut children = values.split_off(values.len() - child_count);
          let args = children.drain(..arg_count).collect();
          let return_type = children.remove(0);
          let rest_type = has_rest.then(|| children.remove(0));
          values.push(Arc::new(Self::Fn(Arc::new(CalcitFnTypeAnnotation {
            generics: signature.generics.clone(),
            where_bounds: signature.where_bounds.clone(),
            arg_types: args,
            return_type,
            fn_kind: signature.fn_kind,
            rest_type,
            features: signature.features.clone(),
          }))));
        }
      }
    }
    values.pop().expect("substitution root result")
  }

  /// Check whether this annotation contains any `TypeVar`.
  pub fn contains_type_var(&self) -> bool {
    let mut pending = vec![self];
    while let Some(annotation) = pending.pop() {
      match annotation {
        Self::TypeVar(_) => return true,
        Self::TypeRef(_, args) | Self::Struct(_, args) | Self::Enum(_, args) => {
          pending.extend(args.iter().map(Arc::as_ref));
        }
        Self::List(inner)
        | Self::Set(inner)
        | Self::Ref(inner)
        | Self::Optional(inner)
        | Self::JsNullish(inner)
        | Self::Variadic(inner) => pending.push(inner),
        Self::Map(key, value) => {
          pending.push(value);
          pending.push(key);
        }
        Self::Fn(signature) => {
          pending.extend(signature.arg_types.iter().map(Arc::as_ref));
          pending.push(&signature.return_type);
          if let Some(rest) = &signature.rest_type {
            pending.push(rest);
          }
        }
        _ => {}
      }
    }
    false
  }

  /// Check whether this annotation recursively contains one named `TypeVar`.
  ///
  /// Generic matching uses this as an occurs-check before recording a binding,
  /// so a value such as `Optional<T>` cannot bind `T` to a type containing
  /// itself and make later comparisons recurse forever.
  fn contains_type_var_named(&self, name: &str) -> bool {
    let mut pending = vec![self];
    while let Some(annotation) = pending.pop() {
      match annotation {
        Self::TypeVar(current) if current.as_ref() == name => return true,
        Self::TypeRef(_, args) | Self::Struct(_, args) | Self::Enum(_, args) => {
          pending.extend(args.iter().map(Arc::as_ref));
        }
        Self::List(inner)
        | Self::Set(inner)
        | Self::Ref(inner)
        | Self::Optional(inner)
        | Self::JsNullish(inner)
        | Self::Variadic(inner) => pending.push(inner),
        Self::Map(key, value) => {
          pending.push(value);
          pending.push(key);
        }
        Self::Fn(signature) => {
          pending.extend(signature.arg_types.iter().map(Arc::as_ref));
          pending.push(&signature.return_type);
          if let Some(rest) = &signature.rest_type {
            pending.push(rest);
          }
        }
        _ => {}
      }
    }
    false
  }

  /// Binding-aware occurs-check used before inserting a generic binding.
  ///
  /// Following existing aliases is necessary to reject indirect cycles such
  /// as `T = U` followed by `U = Optional<T>`. `visiting` also keeps this
  /// defensive check finite if it receives an already-cyclic binding graph.
  fn contains_type_var_named_with_bindings(&self, name: &str, bindings: &TypeBindings) -> bool {
    if self.contains_type_var_named(name) {
      return true;
    }

    fn walk(annotation: &CalcitTypeAnnotation, name: &str, bindings: &TypeBindings, visiting: &mut HashSet<Arc<str>>) -> bool {
      match annotation {
        CalcitTypeAnnotation::TypeVar(current) => {
          if current.as_ref() == name {
            return true;
          }
          if !visiting.insert(current.clone()) {
            return false;
          }
          let found = bindings
            .get(current)
            .is_some_and(|bound| walk(bound.as_ref(), name, bindings, visiting));
          visiting.remove(current);
          found
        }
        CalcitTypeAnnotation::TypeRef(_, args) | CalcitTypeAnnotation::Struct(_, args) | CalcitTypeAnnotation::Enum(_, args) => {
          args.iter().any(|arg| walk(arg, name, bindings, visiting))
        }
        CalcitTypeAnnotation::List(inner)
        | CalcitTypeAnnotation::Set(inner)
        | CalcitTypeAnnotation::Ref(inner)
        | CalcitTypeAnnotation::Optional(inner)
        | CalcitTypeAnnotation::JsNullish(inner)
        | CalcitTypeAnnotation::Variadic(inner) => walk(inner, name, bindings, visiting),
        CalcitTypeAnnotation::Map(key, value) => walk(key, name, bindings, visiting) || walk(value, name, bindings, visiting),
        CalcitTypeAnnotation::Fn(signature) => {
          signature.arg_types.iter().any(|arg| walk(arg, name, bindings, visiting))
            || walk(&signature.return_type, name, bindings, visiting)
            || signature
              .rest_type
              .as_ref()
              .is_some_and(|rest| walk(rest, name, bindings, visiting))
        }
        _ => false,
      }
    }

    walk(self, name, bindings, &mut HashSet::new())
  }

  /// Try to resolve this type annotation to a concrete `CalcitStructDef` definition.
  /// Works for `Struct(def, _)`, `StructValue(def)`, and `TypeRef("ns/name", _)` that can be
  /// looked up from the program registry.
  pub fn resolve_to_struct(&self) -> Option<CalcitStructDef> {
    self.resolve_to_struct_with_ref().map(|(s, _)| s)
  }

  /// Resolve to struct, also returning the (ns, def) path when available from a TypeRef.
  /// The path can be used to construct an Import reference for JS codegen compatibility.
  #[allow(clippy::type_complexity)]
  pub fn resolve_to_struct_with_ref(&self) -> Option<(CalcitStructDef, Option<(Arc<str>, Arc<str>)>)> {
    match self {
      Self::Struct(base, _) => Some((base.as_ref().clone(), split_nominal_definition_ref(base.definition_ref.as_deref()))),
      Self::StructValue(base) => Some((base.as_ref().clone(), split_nominal_definition_ref(base.definition_ref.as_deref()))),
      Self::TypeRef(name, _) => {
        // TypeRef name may be "ns/def" or just "def" — try to split on '/'
        let stripped = name.trim_start_matches('\'').trim_start_matches(':');
        if let Some((ns, def)) = stripped.rsplit_once('/') {
          resolve_struct_from_program(ns, def).map(|s| (s, Some((Arc::from(ns), Arc::from(def)))))
        } else {
          current_type_annotation_namespace()
            .and_then(|ns| resolve_struct_from_program(&ns, stripped).map(|s| (s, Some((ns, Arc::from(stripped))))))
            .or_else(|| resolve_struct_from_program(CORE_NS, stripped).map(|s| (s, Some((Arc::from(CORE_NS), Arc::from(stripped))))))
        }
      }
      Self::Optional(inner) => inner.resolve_to_struct_with_ref(),
      _ => None,
    }
  }

  /// Try to resolve this type annotation to a concrete `CalcitEnumDef` definition.
  /// Works for `Enum(def, _)`, `EnumValue(def)`, and `TypeRef("ns/name", _)` that can be
  /// looked up from the program registry.
  pub fn resolve_to_enum(&self) -> Option<CalcitEnumDef> {
    self.resolve_to_enum_with_ref().map(|(e, _)| e)
  }

  /// Resolve to enum, also returning the (ns, def) path when available from a TypeRef.
  /// The path can be used to construct an Import reference for JS codegen compatibility.
  #[allow(clippy::type_complexity)]
  pub fn resolve_to_enum_with_ref(&self) -> Option<(CalcitEnumDef, Option<(Arc<str>, Arc<str>)>)> {
    match self {
      Self::Enum(base, _) => Some((
        base.as_ref().clone(),
        split_nominal_definition_ref(base.definition_ref().map(AsRef::as_ref)),
      )),
      Self::EnumValue(base) => Some((
        base.as_ref().clone(),
        split_nominal_definition_ref(base.definition_ref().map(AsRef::as_ref)),
      )),
      Self::TypeRef(name, _) => {
        let stripped = name.trim_start_matches('\'').trim_start_matches(':');
        if let Some((ns, def)) = stripped.rsplit_once('/') {
          resolve_enum_from_program(ns, def).map(|e| (e, Some((Arc::from(ns), Arc::from(def)))))
        } else {
          current_type_annotation_namespace()
            .and_then(|ns| resolve_enum_from_program(&ns, stripped).map(|e| (e, Some((ns, Arc::from(stripped))))))
            .or_else(|| {
              // Core named types are commonly written without a namespace in
              // schemas because calcit.core is implicitly available.
              resolve_enum_from_program(CORE_NS, stripped).map(|e| (e, Some((Arc::from(CORE_NS), Arc::from(stripped)))))
            })
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
      Self::Ref(_) => Some("&core-ref-impls"),
      Self::Number => Some("&core-number-impls"),
      Self::DynFn | Self::Fn(_) => Some("&core-fn-impls"),
      Self::Nil | Self::Unit | Self::Bool | Self::Tag | Self::Symbol | Self::CirruQuote => Some("&core-scalar-impls"),
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
      Self::Struct(struct_def, _) | Self::StructValue(struct_def) => Some(struct_def.impls.to_vec()),
      Self::Enum(enum_def, _) | Self::EnumValue(enum_def) => Some(enum_def.impls().to_vec()),
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

  fn impl_matches_trait(imp: &CalcitImpl, expected_trait: &CalcitTrait) -> bool {
    let Some(origin) = imp.origin() else {
      return false;
    };
    if expected_trait.validate_reachable_method_schemas().is_err() || origin.validate_reachable_method_schemas().is_err() {
      return false;
    }
    origin
      .normalized_reachable_traits()
      .is_ok_and(|traits| traits.iter().any(|actual| Self::trait_references_match(actual, expected_trait)))
  }

  fn trait_annotation_proves(actual: &CalcitTrait, expected: &CalcitTrait) -> bool {
    if expected.validate_reachable_method_schemas().is_err() || actual.validate_reachable_method_schemas().is_err() {
      return false;
    }
    actual
      .normalized_reachable_traits()
      .is_ok_and(|traits| traits.iter().any(|reachable| Self::trait_references_match(reachable, expected)))
  }

  /// Bootstrap metadata used only before a core impl list has been evaluated.
  /// Runtime dispatch and evaluated static metadata both use the real impl
  /// origins from calcit-core; this table prevents preprocessing order from
  /// changing whether an otherwise identical core call emits a warning.
  fn builtin_core_trait_names(&self) -> &'static [&'static str] {
    if matches!(self, Self::StructValue(_) | Self::Struct(_, _)) {
      return &["Debug", "Eq", "Countable", "Contains"];
    }
    if matches!(self, Self::AnonymousEnum | Self::EnumValue(_) | Self::Enum(_, _)) {
      return &["Debug", "Eq", "Countable", "Contains"];
    }
    match self.core_impl_list_symbol() {
      Some("&core-list-impls") => &["Debug", "Eq", "Add", "Len", "Mappable", "Countable", "Contains", "Sliceable"],
      Some("&core-map-impls") => &["Debug", "Eq", "Len", "Mappable", "Countable", "Contains"],
      Some("&core-set-impls") => &["Debug", "Eq", "Len", "Mappable", "Countable", "Contains"],
      Some("&core-string-impls") => &["Debug", "Eq", "Add", "Len", "Countable", "Contains", "Compare", "Sliceable"],
      Some("&core-number-impls") => &["Debug", "Eq", "Add", "Multiply", "Compare"],
      Some("&core-fn-impls") => &["Debug"],
      Some("&core-scalar-impls") => &["Debug", "Eq"],
      _ => &[],
    }
  }

  fn satisfies_trait_bound(&self, expected_trait: &CalcitTrait) -> bool {
    match self {
      Self::Trait(actual_trait) if Self::trait_annotation_proves(actual_trait, expected_trait) => {
        return true;
      }
      Self::TraitSet(actual_traits)
        if actual_traits
          .iter()
          .any(|actual_trait| Self::trait_annotation_proves(actual_trait, expected_trait)) =>
      {
        return true;
      }
      _ => {}
    }
    if let Some(impls) = self.collect_static_impls() {
      if impls.iter().any(|imp| Self::impl_matches_trait(imp, expected_trait)) {
        return true;
      }
      // Primitive values use a real core impl list whenever it has finished
      // evaluating. Do not let bootstrap names override that authoritative
      // result; struct/enum categories keep their shared core capabilities in
      // the fallback because their attached impl list is separate.
      if self.core_impl_list_symbol().is_some() {
        return false;
      }
    }
    if !self.builtin_core_trait_names().contains(&expected_trait.name.ref_str()) {
      return false;
    }

    let core_reference = format!("{CORE_NS}/{}", expected_trait.name.ref_str());
    if let Some(definition_ref) = expected_trait.definition_ref.as_deref() {
      return definition_ref == core_reference;
    }

    // Evaluated traits are nominal. If the core trait value is not ready yet,
    // do not let an unrelated runtime trait pass merely because its short name
    // is the same as a built-in capability.
    if expected_trait.runtime_id.is_some() {
      return lookup_runtime_ready_registered(CORE_NS, expected_trait.name.ref_str())
        .and_then(|value| match value {
          Calcit::Trait(core_trait) => Some(core_trait == *expected_trait),
          _ => None,
        })
        .unwrap_or(false);
    }

    // Bare unqualified placeholders are retained only for legacy schemas that
    // predate namespace-qualified symbol references.
    expected_trait.methods.is_empty()
  }

  fn satisfies_trait_bounds(&self, expected_traits: &[Arc<CalcitTrait>]) -> bool {
    expected_traits
      .iter()
      .all(|trait_def| self.satisfies_trait_bound(trait_def.as_ref()))
  }

  pub fn is_compatible_with(&self, expected: &CalcitTypeAnnotation) -> bool {
    let mut bindings = TypeBindings::new();
    self.compatible_with_bindings(expected, &mut bindings)
  }

  pub(crate) fn is_proven_for(&self, expected: &CalcitTypeAnnotation) -> bool {
    let mut bindings = TypeBindings::new();
    self.prove_with_bindings(expected, &mut bindings).is_proven()
  }

  pub(crate) fn compatible_with_bindings(&self, expected: &CalcitTypeAnnotation, bindings: &mut TypeBindings) -> bool {
    let _relation_guard = enter_compatibility_relation();
    match bounded_plain_type_relation(self, expected, PlainRelationMode::Compatibility) {
      Ok(Some((result, _))) => return !result.is_mismatch(),
      Err(_) => return false,
      Ok(None) => {}
    }
    let mut worklist = vec![(self, expected)];
    let mut visited = HashSet::new();
    while let Some((actual, expected)) = worklist.pop() {
      if !visited.insert((actual as *const Self as usize, expected as *const Self as usize)) {
        continue;
      }
      if !consume_compatibility_relation_visit() {
        return false;
      }
      match (actual, expected) {
        (Self::List(actual), Self::List(expected))
        | (Self::Set(actual), Self::Set(expected))
        | (Self::Ref(actual), Self::Ref(expected))
        | (Self::Variadic(actual), Self::Variadic(expected))
        | (Self::Optional(actual), Self::Optional(expected))
        | (Self::JsNullish(actual), Self::JsNullish(expected)) => {
          worklist.push((actual.as_ref(), expected.as_ref()));
        }
        (Self::Map(actual_key, actual_value), Self::Map(expected_key, expected_value)) => {
          worklist.push((actual_value.as_ref(), expected_value.as_ref()));
          worklist.push((actual_key.as_ref(), expected_key.as_ref()));
        }
        (Self::TypeRef(actual_name, actual_args), Self::TypeRef(expected_name, expected_args))
          if actual_args.len() == expected_args.len()
            && !actual_args.is_empty()
            && Self::type_ref_nominal_match(actual_name, expected_name).unwrap_or_else(|| {
              Self::type_ref_name_matches(actual_name, expected_name) || Self::type_ref_name_matches(expected_name, actual_name)
            }) =>
        {
          worklist.extend(
            actual_args
              .iter()
              .zip(expected_args.iter())
              .rev()
              .map(|(actual, expected)| (actual.as_ref(), expected.as_ref())),
          );
        }
        (Self::Struct(actual_base, actual_args), Self::Struct(expected_base, expected_args))
          if actual_args.len() == expected_args.len()
            && !actual_args.is_empty()
            && Self::struct_nominal_match(actual_base, expected_base).unwrap_or_else(|| actual_base.name == expected_base.name) =>
        {
          worklist.extend(
            actual_args
              .iter()
              .zip(expected_args.iter())
              .rev()
              .map(|(actual, expected)| (actual.as_ref(), expected.as_ref())),
          );
        }
        (Self::Enum(actual_base, actual_args), Self::Enum(expected_base, expected_args))
          if actual_args.len() == expected_args.len()
            && !actual_args.is_empty()
            && Self::enum_nominal_match(actual_base, expected_base).unwrap_or_else(|| actual_base.name() == expected_base.name()) =>
        {
          worklist.extend(
            actual_args
              .iter()
              .zip(expected_args.iter())
              .rev()
              .map(|(actual, expected)| (actual.as_ref(), expected.as_ref())),
          );
        }
        _ if actual.compatible_one_with_bindings(expected, bindings) => {}
        _ => return false,
      }
    }
    true
  }

  fn compatible_one_with_bindings(&self, expected: &CalcitTypeAnnotation, bindings: &mut TypeBindings) -> bool {
    match (self, expected) {
      (_, Self::Dynamic) | (Self::Dynamic, _) => true,
      (Self::Macro(actual), Self::Macro(expected)) => actual == expected,
      (Self::Syntax(actual), Self::Syntax(expected)) => actual == expected,
      // Compatibility for annotations constructed by older embedders before
      // `:any` was normalized during parsing. Alias semantics are symmetric.
      (_, Self::Custom(expected)) if Self::custom_keyword_matches(expected, "any") => true,
      (Self::Custom(actual), _) if Self::custom_keyword_matches(actual, "any") => true,
      (Self::TypeVar(actual), Self::TypeVar(expected)) if actual == expected => true,
      (actual, Self::TypeVar(var)) => match bindings.get(var) {
        Some(bound) if bound.as_ref() == actual => true,
        Some(bound) if matches!(bound.as_ref(), Self::Nil) => {
          let merged = if matches!(actual, Self::Optional(_)) {
            actual.to_owned()
          } else {
            Self::Optional(Arc::new(actual.to_owned()))
          };
          bindings.insert(var.to_owned(), Arc::new(merged));
          true
        }
        Some(bound) if matches!(actual, Self::Nil) => {
          if !matches!(bound.as_ref(), Self::Optional(_)) {
            bindings.insert(var.to_owned(), Arc::new(Self::Optional(bound.clone())));
          }
          true
        }
        Some(bound) => {
          let bound = bound.clone();
          actual.compatible_with_bindings(bound.as_ref(), bindings)
        }
        None if actual.contains_type_var_named_with_bindings(var, bindings) => true,
        None => {
          bindings.insert(var.to_owned(), Arc::new(actual.to_owned()));
          true
        }
      },
      (_, Self::Optional(expected_inner)) => match self {
        Self::Optional(actual_inner) => actual_inner.compatible_with_bindings(expected_inner, bindings),
        Self::JsNullish(_) => false,
        Self::Nil => true,
        _ => self.compatible_with_bindings(expected_inner, bindings),
      },
      (Self::Optional(_), _) => false,
      (_, Self::JsNullish(expected_inner)) => match self {
        Self::JsNullish(actual_inner) => actual_inner.compatible_with_bindings(expected_inner, bindings),
        Self::Optional(_) => false,
        Self::Nil => true,
        _ => self.compatible_with_bindings(expected_inner, bindings),
      },
      (Self::JsNullish(_), _) => false,
      (Self::Bool, Self::Bool)
      | (Self::Number, Self::Number)
      | (Self::String, Self::String)
      | (Self::Symbol, Self::Symbol)
      | (Self::Tag, Self::Tag)
      | (Self::DynFn, Self::DynFn)
      | (Self::Buffer, Self::Buffer)
      | (Self::F64Buffer, Self::F64Buffer)
      | (Self::CirruQuote, Self::CirruQuote)
      | (Self::JsObject, Self::JsObject)
      | (Self::Nil, Self::Nil)
      | (Self::Unit, Self::Unit) => true,
      (Self::TypeVar(var), expected_type) => match bindings.get(var) {
        Some(bound) if bound.as_ref() == expected_type => true,
        Some(bound) if matches!(bound.as_ref(), Self::Nil) => {
          let merged = if matches!(expected_type, Self::Optional(_)) {
            expected_type.to_owned()
          } else {
            Self::Optional(Arc::new(expected_type.to_owned()))
          };
          bindings.insert(var.to_owned(), Arc::new(merged));
          true
        }
        Some(bound) if matches!(expected_type, Self::Nil) => {
          if !matches!(bound.as_ref(), Self::Optional(_)) {
            bindings.insert(var.to_owned(), Arc::new(Self::Optional(bound.clone())));
          }
          true
        }
        Some(bound) => {
          let bound = bound.clone();
          bound.as_ref().compatible_with_bindings(expected_type, bindings)
        }
        None if expected_type.contains_type_var_named_with_bindings(var, bindings) => true,
        None => {
          bindings.insert(var.to_owned(), Arc::new(expected_type.to_owned()));
          true
        }
      },
      (Self::TypeRef(a_name, a_args), Self::TypeRef(b_name, b_args)) => {
        let nominal_matches = Self::type_ref_nominal_match(a_name, b_name)
          .unwrap_or_else(|| Self::type_ref_name_matches(a_name, b_name) || Self::type_ref_name_matches(b_name, a_name));
        if !nominal_matches {
          return false;
        }
        if a_args.is_empty() || b_args.is_empty() {
          return true;
        }
        a_args.len() == b_args.len()
          && a_args
            .iter()
            .zip(b_args.iter())
            .all(|(x, y)| x.compatible_with_bindings(y, bindings))
      }
      (Self::List(a), Self::List(b)) => a.compatible_with_bindings(b, bindings),
      (Self::Map(ak, av), Self::Map(bk, bv)) => ak.compatible_with_bindings(bk, bindings) && av.compatible_with_bindings(bv, bindings),
      (Self::Set(a), Self::Set(b)) => a.compatible_with_bindings(b, bindings),
      (Self::Ref(a), Self::Ref(b)) => a.compatible_with_bindings(b, bindings),
      (Self::TypeRef(name, args), Self::Struct(base, other_args)) | (Self::Struct(base, other_args), Self::TypeRef(name, args)) => {
        let nominal_matches = Self::type_ref_matches_struct_definition(name, base).unwrap_or_else(|| {
          Self::type_ref_name_matches(name, base.name.ref_str()) || Self::type_ref_resolves_to_struct_name(name, base.name.ref_str())
        });
        if !nominal_matches {
          return false;
        }
        match (args.is_empty(), other_args.is_empty()) {
          (true, true) => true,
          (false, false) => {
            args.len() == other_args.len()
              && args
                .iter()
                .zip(other_args.iter())
                .all(|(x, y)| x.compatible_with_bindings(y, bindings))
          }
          (true, false) => Self::bind_declared_generics_from_applied_args(base.generics.as_ref(), other_args.as_ref(), bindings),
          (false, true) => Self::bind_declared_generics_from_applied_args(base.generics.as_ref(), args.as_ref(), bindings),
        }
      }
      (Self::TypeRef(name, args), Self::Enum(base, other_args)) | (Self::Enum(base, other_args), Self::TypeRef(name, args)) => {
        let nominal_matches = Self::type_ref_matches_enum_definition(name, base).unwrap_or_else(|| {
          Self::type_ref_name_matches(name, base.name().ref_str()) || Self::type_ref_resolves_to_enum_name(name, base.name().ref_str())
        });
        if !nominal_matches {
          return false;
        }
        match (args.is_empty(), other_args.is_empty()) {
          (true, true) => true,
          (false, false) => {
            args.len() == other_args.len()
              && args
                .iter()
                .zip(other_args.iter())
                .all(|(x, y)| x.compatible_with_bindings(y, bindings))
          }
          (true, false) => Self::bind_declared_generics_from_applied_args(base.generics(), other_args.as_ref(), bindings),
          (false, true) => Self::bind_declared_generics_from_applied_args(base.generics(), args.as_ref(), bindings),
        }
      }
      (Self::TypeRef(name, _), Self::StructValue(base)) | (Self::StructValue(base), Self::TypeRef(name, _)) => {
        // Structs are structurally map-like (field name -> value), so procs typed as
        // accepting a generic "map" (e.g. `to-pairs`/`keys`) should also accept structs,
        // in addition to matching the struct's own type name.
        Self::type_ref_name_matches(name, "map")
          || Self::type_ref_matches_struct_definition(name, base).unwrap_or_else(|| {
            Self::type_ref_name_matches(name, base.name.ref_str()) || Self::type_ref_resolves_to_struct_name(name, base.name.ref_str())
          })
      }
      (Self::TypeRef(name, _), Self::EnumValue(base)) | (Self::EnumValue(base), Self::TypeRef(name, _)) => {
        Self::type_ref_matches_enum_definition(name, base).unwrap_or_else(|| {
          Self::type_ref_name_matches(name, base.name().ref_str()) || Self::type_ref_resolves_to_enum_name(name, base.name().ref_str())
        })
      }
      (Self::Struct(a, a_args), Self::Struct(b, b_args)) => {
        if !Self::struct_nominal_match(a, b).unwrap_or_else(|| a.name == b.name) {
          return false;
        }
        match (a_args.is_empty(), b_args.is_empty()) {
          (true, true) => true,
          (false, false) => {
            a_args.len() == b_args.len()
              && a_args
                .iter()
                .zip(b_args.iter())
                .all(|(x, y)| x.compatible_with_bindings(y, bindings))
          }
          _ => {
            // one applied, one bare — bind generics from the applied side
            let (base, args) = if !a_args.is_empty() { (a, a_args) } else { (b, b_args) };
            Self::bind_declared_generics_from_applied_args(base.generics.as_ref(), args.as_ref(), bindings)
          }
        }
      }
      (Self::Enum(a, a_args), Self::Enum(b, b_args)) => {
        if !Self::enum_nominal_match(a, b).unwrap_or_else(|| a.name() == b.name()) {
          return false;
        }
        if a_args.is_empty() || b_args.is_empty() {
          return true;
        }
        a_args.len() == b_args.len()
          && a_args
            .iter()
            .zip(b_args.iter())
            .all(|(x, y)| x.compatible_with_bindings(y, bindings))
      }
      (Self::TypeRef(name, args), Self::Trait(trait_def)) | (Self::Trait(trait_def), Self::TypeRef(name, args)) => {
        args.is_empty() && Self::type_ref_matches_trait(name, trait_def.as_ref())
      }
      (Self::Trait(a), Self::Trait(b)) => Self::trait_references_match(a, b),
      (Self::TraitSet(actual), Self::Trait(expected)) => {
        actual.iter().any(|trait_def| Self::trait_references_match(trait_def, expected))
      }
      (Self::Trait(actual), Self::TraitSet(expected)) => {
        expected.len() == 1 && expected.iter().any(|trait_def| Self::trait_references_match(actual, trait_def))
      }
      (Self::TraitSet(actual), Self::TraitSet(expected)) => expected.iter().all(|expected_trait| {
        actual
          .iter()
          .any(|actual_trait| Self::trait_references_match(actual_trait, expected_trait))
      }),
      (actual, Self::Trait(expected)) => actual.satisfies_trait_bound(expected.as_ref()),
      (actual, Self::TraitSet(expected)) => actual.satisfies_trait_bounds(expected.as_ref()),
      (Self::Struct(_, _), Self::Custom(expected))
        if Self::custom_keyword_matches(expected, "struct") || Self::custom_keyword_matches(expected, "record") =>
      {
        true
      }
      (Self::Enum(_, _), Self::Custom(expected))
        if Self::custom_keyword_matches(expected, "enum") || Self::custom_keyword_matches(expected, "tuple") =>
      {
        true
      }
      (Self::TypeRef(_, _), Self::Custom(expected)) if Self::custom_keyword_matches(expected, "struct-def") => {
        self.resolve_to_struct().is_some()
      }
      (Self::TypeRef(_, _), Self::Custom(expected)) if Self::custom_keyword_matches(expected, "enum-def") => {
        self.resolve_to_enum().is_some()
      }
      (Self::StructDef(actual), Self::StructDef(expected)) => {
        Self::struct_nominal_match(actual, expected).unwrap_or_else(|| actual.name == expected.name)
      }
      (Self::EnumDef(actual), Self::EnumDef(expected)) => {
        Self::enum_nominal_match(actual, expected).unwrap_or_else(|| actual.name() == expected.name())
      }
      (Self::StructDef(_), Self::Custom(expected)) if Self::custom_keyword_matches(expected, "struct-def") => true,
      (Self::EnumDef(_), Self::Custom(expected)) if Self::custom_keyword_matches(expected, "enum-def") => true,
      (Self::Trait(_), Self::Custom(expected)) if Self::custom_keyword_matches(expected, "trait") => true,
      (Self::TraitSet(_), Self::Custom(expected)) if Self::custom_keyword_matches(expected, "trait") => true,
      (Self::Custom(actual), Self::Custom(expected))
        if Self::custom_keyword_matches(expected, "impl") && matches!(actual.as_ref(), Calcit::Impl(_)) =>
      {
        true
      }
      // AnonymousEnum matches any other AnonymousEnum
      (Self::AnonymousEnum, Self::AnonymousEnum) => true,
      // Function type matching: DynFn matches any Fn, specific Fn must match signature
      (Self::Fn(_), Self::DynFn) | (Self::DynFn, Self::Fn(_)) => true,
      // Tags are callable in Calcit (as map key accessors), so they satisfy :fn requirements
      (Self::Tag, Self::DynFn) | (Self::Tag, Self::Fn(_)) => true,
      (Self::Fn(a), Self::Fn(b)) => a.compatible_signature_with_bindings(b.as_ref(), bindings),
      (Self::Variadic(a), Self::Variadic(b)) => a.compatible_with_bindings(b, bindings),
      (Self::Custom(a), Self::Custom(b)) => a.as_ref() == b.as_ref(),
      (Self::StructValue(a), Self::StructValue(b)) => Self::struct_nominal_match(a, b).unwrap_or_else(|| a.name == b.name),
      (Self::StructValue(a), Self::Struct(b, _)) => Self::struct_nominal_match(a, b).unwrap_or_else(|| a.name == b.name),
      (Self::StructValue(_), Self::Custom(expected))
        if Self::custom_keyword_matches(expected, "record") || Self::custom_keyword_matches(expected, "struct") =>
      {
        true
      }
      (Self::EnumValue(a), Self::EnumValue(b)) => Self::enum_nominal_match(a, b).unwrap_or_else(|| a.name() == b.name()),
      (Self::EnumValue(a), Self::Enum(b, _)) | (Self::Enum(b, _), Self::EnumValue(a)) => {
        Self::enum_nominal_match(a, b).unwrap_or_else(|| a.name() == b.name())
      }
      (Self::EnumValue(_), Self::Custom(expected))
        if Self::custom_keyword_matches(expected, "tuple") || Self::custom_keyword_matches(expected, "enum") =>
      {
        true
      }
      (Self::EnumValue(_), Self::AnonymousEnum) | (Self::AnonymousEnum, Self::EnumValue(_)) => true,
      // Enum values use a tagged runtime representation, so a concrete named
      // enum is safe wherever a builtin proc accepts a dynamic enum. Keep the
      // relation directional: a dynamic enum cannot satisfy a named enum.
      (Self::Enum(_, _), Self::AnonymousEnum) => true,
      (type_ref @ Self::TypeRef(_, _), Self::AnonymousEnum) => type_ref.resolve_to_enum().is_some(),
      (type_ref @ Self::TypeRef(_, _), Self::Custom(expected)) | (Self::Custom(expected), type_ref @ Self::TypeRef(_, _))
        if Self::custom_keyword_matches(expected, "record") || Self::custom_keyword_matches(expected, "struct") =>
      {
        type_ref.resolve_to_struct().is_some()
      }
      (type_ref @ Self::TypeRef(_, _), Self::Custom(expected)) | (Self::Custom(expected), type_ref @ Self::TypeRef(_, _))
        if Self::custom_keyword_matches(expected, "tuple") || Self::custom_keyword_matches(expected, "enum") =>
      {
        type_ref.resolve_to_enum().is_some()
      }
      // TypeRef schema resolution: when a TypeRef doesn't match any concrete type above,
      // try to resolve it as a type alias by looking up the definition's schema.
      (Self::TypeRef(name, _), other) => {
        if let Some(resolved) = resolve_type_ref_as_schema(name) {
          let Ok(_guard) = enter_alias_relation(name, other, true, false) else {
            return false;
          };
          return resolved.compatible_with_bindings(other, bindings);
        }
        false
      }
      (other, Self::TypeRef(name, _)) => {
        if let Some(resolved) = resolve_type_ref_as_schema(name) {
          let Ok(_guard) = enter_alias_relation(name, other, false, false) else {
            return false;
          };
          return other.compatible_with_bindings(resolved.as_ref(), bindings);
        }
        false
      }
      // TypeSlot: resolve the bound type from the global registry and delegate
      (Self::TypeSlot(name), other) | (other, Self::TypeSlot(name)) => {
        if let Some(resolved) = resolve_type_slot(name) {
          return with_type_relation_symbol(&TYPE_RELATION_SLOT_STACK, name, || {
            resolved.compatible_with_bindings(other, bindings)
          })
          .unwrap_or(false);
        }
        // Slot not yet bound — treat as Dynamic (no checking)
        true
      }
      _ => false,
    }
  }

  /// Prove that `self` satisfies `expected` using only concrete static
  /// evidence. Bindings are committed atomically only for a complete proof.
  /// Legacy compatibility boundaries remain observable as `NeedsBoundary`.
  pub(crate) fn prove_with_bindings(&self, expected: &CalcitTypeAnnotation, bindings: &mut TypeBindings) -> TypeProof {
    let mut staged = bindings.clone();
    let result = self.prove_with_staged_bindings(expected, &mut staged);
    if result.is_proven() {
      *bindings = staged;
    }
    result
  }

  /// Collect concrete bindings that were proven before an unrelated boundary
  /// was encountered. A mismatch still rolls the whole relation back. This is
  /// intended for output projections such as `Result<T, Dynamic> -> T`.
  pub(crate) fn prove_available_bindings(&self, expected: &CalcitTypeAnnotation, bindings: &mut TypeBindings) -> TypeProof {
    let mut staged = bindings.clone();
    let result = self.prove_with_staged_bindings(expected, &mut staged);
    if !result.is_mismatch() {
      *bindings = staged;
    }
    result
  }

  fn prove_with_staged_bindings(&self, expected: &CalcitTypeAnnotation, bindings: &mut TypeBindings) -> TypeProof {
    use TypeBoundaryReason as Boundary;
    use TypeProof::{Mismatch, NeedsBoundary, Proven};

    match bounded_plain_type_relation(self, expected, PlainRelationMode::Proof) {
      Ok(Some((result, _))) => return result,
      Err(_) => return NeedsBoundary(Boundary::TypeComplexityLimit),
      Ok(None) => {}
    }

    match (self, expected) {
      (_, Self::Dynamic) | (Self::Dynamic, _) => NeedsBoundary(Boundary::Dynamic),
      (_, Self::Custom(value)) if Self::custom_keyword_matches(value, "any") => NeedsBoundary(Boundary::LegacyAny),
      (Self::Custom(value), _) if Self::custom_keyword_matches(value, "any") => NeedsBoundary(Boundary::LegacyAny),
      (Self::TypeVar(actual), Self::TypeVar(expected)) if actual == expected => Proven,
      (actual, Self::TypeVar(var)) => match bindings.get(var).cloned() {
        Some(bound) => actual.prove_with_staged_bindings(bound.as_ref(), bindings),
        None if actual.contains_type_var_named_with_bindings(var, bindings) => NeedsBoundary(Boundary::RecursiveTypeVariable),
        None => {
          bindings.insert(var.clone(), Arc::new(actual.clone()));
          Proven
        }
      },
      (Self::TypeVar(var), expected_type) => match bindings.get(var).cloned() {
        Some(bound) => bound.prove_with_staged_bindings(expected_type, bindings),
        None if expected_type.contains_type_var_named_with_bindings(var, bindings) => NeedsBoundary(Boundary::RecursiveTypeVariable),
        None => {
          bindings.insert(var.clone(), Arc::new(expected_type.clone()));
          Proven
        }
      },
      (Self::Optional(actual), Self::Optional(expected)) | (Self::JsNullish(actual), Self::JsNullish(expected)) => {
        actual.prove_with_staged_bindings(expected, bindings)
      }
      (_, Self::Optional(_)) | (Self::Optional(_), _) | (_, Self::JsNullish(_)) | (Self::JsNullish(_), _) => {
        if self.compatible_with_bindings(expected, &mut bindings.clone()) {
          NeedsBoundary(Boundary::LegacyNullish)
        } else {
          Mismatch
        }
      }
      (Self::TypeRef(a_name, a_args), Self::TypeRef(b_name, b_args)) => {
        match Self::type_ref_nominal_match(a_name, b_name) {
          Some(true) => {}
          Some(false) => return Mismatch,
          None => return NeedsBoundary(Boundary::UnresolvedNominalIdentity),
        }
        if a_args.is_empty() != b_args.is_empty() {
          return NeedsBoundary(Boundary::ErasedTypeArguments);
        }
        Self::prove_slices(a_args, b_args, bindings)
      }
      (Self::TypeRef(name, actual_args), Self::Struct(base, expected_args)) => {
        match Self::type_ref_matches_struct_definition(name, base) {
          Some(true) => {}
          Some(false) => return Mismatch,
          None => return NeedsBoundary(Boundary::UnresolvedNominalIdentity),
        }
        match (actual_args.is_empty(), expected_args.is_empty()) {
          (true, true) => Proven,
          (false, false) => Self::prove_slices(actual_args, expected_args, bindings),
          _ => NeedsBoundary(Boundary::ErasedTypeArguments),
        }
      }
      (Self::Struct(base, actual_args), Self::TypeRef(name, expected_args)) => {
        match Self::type_ref_matches_struct_definition(name, base) {
          Some(true) => {}
          Some(false) => return Mismatch,
          None => return NeedsBoundary(Boundary::UnresolvedNominalIdentity),
        }
        match (actual_args.is_empty(), expected_args.is_empty()) {
          (true, true) => Proven,
          (false, false) => Self::prove_slices(actual_args, expected_args, bindings),
          _ => NeedsBoundary(Boundary::ErasedTypeArguments),
        }
      }
      (Self::TypeRef(name, actual_args), Self::Enum(base, expected_args)) => {
        match Self::type_ref_matches_enum_definition(name, base) {
          Some(true) => {}
          Some(false) => return Mismatch,
          None => return NeedsBoundary(Boundary::UnresolvedNominalIdentity),
        }
        match (actual_args.is_empty(), expected_args.is_empty()) {
          (true, true) => Proven,
          (false, false) => Self::prove_slices(actual_args, expected_args, bindings),
          _ => NeedsBoundary(Boundary::ErasedTypeArguments),
        }
      }
      (Self::Enum(base, actual_args), Self::TypeRef(name, expected_args)) => {
        match Self::type_ref_matches_enum_definition(name, base) {
          Some(true) => {}
          Some(false) => return Mismatch,
          None => return NeedsBoundary(Boundary::UnresolvedNominalIdentity),
        }
        match (actual_args.is_empty(), expected_args.is_empty()) {
          (true, true) => Proven,
          (false, false) => Self::prove_slices(actual_args, expected_args, bindings),
          _ => NeedsBoundary(Boundary::ErasedTypeArguments),
        }
      }
      (Self::TypeRef(name, args), Self::StructValue(base)) | (Self::StructValue(base), Self::TypeRef(name, args)) => {
        match Self::type_ref_matches_struct_definition(name, base) {
          Some(false) => Mismatch,
          Some(true) if args.is_empty() && base.generics.is_empty() => Proven,
          Some(true) => NeedsBoundary(Boundary::ErasedTypeArguments),
          None => NeedsBoundary(Boundary::UnresolvedNominalIdentity),
        }
      }
      (Self::TypeRef(name, args), Self::EnumValue(base)) | (Self::EnumValue(base), Self::TypeRef(name, args)) => {
        match Self::type_ref_matches_enum_definition(name, base) {
          Some(false) => Mismatch,
          Some(true) if args.is_empty() && base.generics().is_empty() => Proven,
          Some(true) => NeedsBoundary(Boundary::ErasedTypeArguments),
          None => NeedsBoundary(Boundary::UnresolvedNominalIdentity),
        }
      }
      (Self::List(actual), Self::List(expected))
      | (Self::Set(actual), Self::Set(expected))
      | (Self::Variadic(actual), Self::Variadic(expected)) => actual.prove_with_staged_bindings(expected, bindings),
      (Self::Ref(actual), Self::Ref(expected)) => {
        let mut invariant_bindings = bindings.clone();
        let result = actual
          .prove_with_staged_bindings(expected, &mut invariant_bindings)
          .and(expected.prove_with_staged_bindings(actual, &mut invariant_bindings));
        if result.is_proven() {
          *bindings = invariant_bindings;
        }
        result
      }
      (Self::Map(actual_key, actual_value), Self::Map(expected_key, expected_value)) => actual_key
        .prove_with_staged_bindings(expected_key, bindings)
        .and(actual_value.prove_with_staged_bindings(expected_value, bindings)),
      (Self::Struct(actual, actual_args), Self::Struct(expected, expected_args)) => {
        match Self::struct_nominal_match(actual, expected) {
          Some(true) => {}
          Some(false) => return Mismatch,
          None => return NeedsBoundary(Boundary::UnresolvedNominalIdentity),
        }
        if actual_args.is_empty() != expected_args.is_empty() {
          NeedsBoundary(Boundary::ErasedTypeArguments)
        } else {
          Self::prove_slices(actual_args, expected_args, bindings)
        }
      }
      (Self::Enum(actual, actual_args), Self::Enum(expected, expected_args)) => {
        match Self::enum_nominal_match(actual, expected) {
          Some(true) => {}
          Some(false) => return Mismatch,
          None => return NeedsBoundary(Boundary::UnresolvedNominalIdentity),
        }
        if actual_args.is_empty() != expected_args.is_empty() {
          NeedsBoundary(Boundary::ErasedTypeArguments)
        } else {
          Self::prove_slices(actual_args, expected_args, bindings)
        }
      }
      (Self::StructValue(actual), Self::StructValue(expected)) => match Self::struct_nominal_match(actual, expected) {
        Some(true) => Proven,
        Some(false) => Mismatch,
        None => NeedsBoundary(Boundary::UnresolvedNominalIdentity),
      },
      (Self::StructValue(actual), Self::Struct(expected, expected_args)) => match Self::struct_nominal_match(actual, expected) {
        Some(false) => Mismatch,
        Some(true) if expected_args.is_empty() && actual.generics.is_empty() => Proven,
        Some(true) => NeedsBoundary(Boundary::ErasedTypeArguments),
        None => NeedsBoundary(Boundary::UnresolvedNominalIdentity),
      },
      (Self::Struct(actual, actual_args), Self::StructValue(expected)) => match Self::struct_nominal_match(actual, expected) {
        Some(false) => Mismatch,
        Some(true) if actual_args.is_empty() && expected.generics.is_empty() => Proven,
        Some(true) => NeedsBoundary(Boundary::ErasedTypeArguments),
        None => NeedsBoundary(Boundary::UnresolvedNominalIdentity),
      },
      (Self::EnumValue(actual), Self::EnumValue(expected)) => match Self::enum_nominal_match(actual, expected) {
        Some(true) => Proven,
        Some(false) => Mismatch,
        None => NeedsBoundary(Boundary::UnresolvedNominalIdentity),
      },
      (Self::EnumValue(actual), Self::Enum(expected, expected_args))
      | (Self::Enum(expected, expected_args), Self::EnumValue(actual)) => match Self::enum_nominal_match(actual, expected) {
        Some(false) => Mismatch,
        Some(true) if expected_args.is_empty() && actual.generics().is_empty() => Proven,
        Some(true) => NeedsBoundary(Boundary::ErasedTypeArguments),
        None => NeedsBoundary(Boundary::UnresolvedNominalIdentity),
      },
      (Self::StructDef(actual), Self::StructDef(expected)) => match Self::struct_nominal_match(actual, expected) {
        Some(true) => Proven,
        Some(false) => Mismatch,
        None => NeedsBoundary(Boundary::UnresolvedNominalIdentity),
      },
      (Self::EnumDef(actual), Self::EnumDef(expected)) => match Self::enum_nominal_match(actual, expected) {
        Some(true) => Proven,
        Some(false) => Mismatch,
        None => NeedsBoundary(Boundary::UnresolvedNominalIdentity),
      },
      (Self::Fn(actual), Self::Fn(expected)) => actual.prove_signature_with_bindings(expected, bindings),
      (Self::Fn(_), Self::DynFn) | (Self::DynFn, Self::Fn(_)) => NeedsBoundary(Boundary::UnknownCallable),
      (Self::Tag, Self::DynFn) | (Self::Tag, Self::Fn(_)) => NeedsBoundary(Boundary::TagCallable),
      (Self::TypeSlot(name), other) => match resolve_type_slot(name) {
        Some(resolved) => with_type_relation_symbol(&TYPE_RELATION_SLOT_STACK, name, || {
          resolved.prove_with_staged_bindings(other, bindings)
        })
        .unwrap_or(NeedsBoundary(Boundary::RecursiveTypeSlot)),
        None => NeedsBoundary(Boundary::UnresolvedTypeSlot),
      },
      (other, Self::TypeSlot(name)) => match resolve_type_slot(name) {
        Some(resolved) => with_type_relation_symbol(&TYPE_RELATION_SLOT_STACK, name, || {
          other.prove_with_staged_bindings(resolved.as_ref(), bindings)
        })
        .unwrap_or(NeedsBoundary(Boundary::RecursiveTypeSlot)),
        None => NeedsBoundary(Boundary::UnresolvedTypeSlot),
      },
      (Self::TypeRef(name, _), other) => match resolve_type_ref_as_schema(name) {
        Some(resolved) => match enter_alias_relation(name, other, true, true) {
          Ok(_guard) => resolved.prove_with_staged_bindings(other, bindings),
          Err(reason) => NeedsBoundary(reason),
        },
        None => Mismatch,
      },
      (other, Self::TypeRef(name, _)) => match resolve_type_ref_as_schema(name) {
        Some(resolved) => match enter_alias_relation(name, other, false, true) {
          Ok(_guard) => other.prove_with_staged_bindings(resolved.as_ref(), bindings),
          Err(reason) => NeedsBoundary(reason),
        },
        None => Mismatch,
      },
      _ => {
        if self.compatible_with_bindings(expected, &mut bindings.clone()) {
          Proven
        } else {
          Mismatch
        }
      }
    }
  }

  fn prove_slices(
    actual: &[Arc<CalcitTypeAnnotation>],
    expected: &[Arc<CalcitTypeAnnotation>],
    bindings: &mut TypeBindings,
  ) -> TypeProof {
    if actual.len() != expected.len() {
      return TypeProof::Mismatch;
    }
    actual.iter().zip(expected).fold(TypeProof::Proven, |result, (actual, expected)| {
      result.and(actual.prove_with_staged_bindings(expected, bindings))
    })
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
      Calcit::List(_) => Self::List(DYNAMIC_TYPE.clone()),
      Calcit::Map(_) => Self::Map(DYNAMIC_TYPE.clone(), DYNAMIC_TYPE.clone()),
      Calcit::Set(_) => Self::Set(DYNAMIC_TYPE.clone()),
      Calcit::Struct(struct_value) => Self::StructValue(struct_value.struct_ref.clone()),
      Calcit::EnumDef(enum_def) => Self::EnumDef(Arc::new(enum_def.to_owned())),
      Calcit::StructDef(struct_def) => Self::StructDef(Arc::new(struct_def.to_owned())),
      Calcit::Enum(enum_value) => {
        // Check for special enum patterns
        if let Calcit::Tag(tag) = enum_value.tag.as_ref() {
          let tag_name = tag.ref_str().trim_start_matches(':');
          if tag_name == "&" && enum_value.extra.len() == 1 {
            // Variadic type: (& :type)
            return Self::Variadic(Arc::new(Self::from_calcit(&enum_value.extra[0])));
          } else if tag_name == "optional" && enum_value.extra.len() == 1 {
            // Optional type: (optional :type)
            return Self::Optional(Arc::new(Self::from_calcit(&enum_value.extra[0])));
          } else if tag_name == "js-nullish" && enum_value.extra.len() == 1 {
            return Self::JsNullish(Arc::new(Self::from_calcit(&enum_value.extra[0])));
          }
        }
        match &enum_value.sum_type {
          Some(enum_def) => Self::EnumValue(enum_def.clone()),
          None => Self::AnonymousEnum,
        }
      }
      Calcit::Fn { info, .. } => Self::from_calcit_fn(info),
      Calcit::Import(import) => Self::from_import(import).unwrap_or(Self::Dynamic),
      Calcit::Proc(proc) => {
        if let Some(signature) = proc.get_type_signature() {
          Self::from_function_parts(signature.arg_types.clone(), signature.return_type.clone())
        } else {
          Self::Dynamic
        }
      }
      Calcit::Ref(_, _) => Self::Ref(DYNAMIC_TYPE.clone()),
      Calcit::Symbol { .. } => Self::Symbol,
      Calcit::Buffer(_) => Self::Buffer,
      Calcit::F64Buffer(_) => Self::F64Buffer,
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
      Self::builtin_type_from_tag_name(tag_name)
        .or_else(|| Self::builtin_type_from_symbol_name(tag_name))
        .unwrap_or_else(|| Self::Tag)
    }
  }

  pub fn from_function_parts(arg_types: Vec<Arc<CalcitTypeAnnotation>>, return_type: Arc<CalcitTypeAnnotation>) -> Self {
    let mut fixed_arg_types = arg_types;
    let rest_type = fixed_arg_types.last().and_then(|last| match last.as_ref() {
      Self::Variadic(inner) => Some(inner.clone()),
      _ => None,
    });
    if rest_type.is_some() {
      fixed_arg_types.pop();
    }
    Self::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: fixed_arg_types,
      return_type,
      fn_kind: SchemaKind::Fn,
      rest_type,
      features: Arc::new(HashSet::new()),
    }))
  }

  /// Preserve the complete callable schema carried by a runtime function.
  pub fn from_calcit_fn(info: &CalcitFn) -> Self {
    let mut fixed_arg_types = info.arg_types.clone();
    let trailing_rest = fixed_arg_types.last().and_then(|last| match last.as_ref() {
      Self::Variadic(inner) => Some(inner.clone()),
      _ => None,
    });
    if trailing_rest.is_some() {
      fixed_arg_types.pop();
    }
    Self::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: info.generics.clone(),
      where_bounds: info.where_bounds.clone(),
      arg_types: fixed_arg_types,
      return_type: info.return_type.clone(),
      fn_kind: SchemaKind::Fn,
      rest_type: info.rest_type.clone().or(trailing_rest),
      features: Arc::new(HashSet::new()),
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
      Self::Variadic(inner) => Calcit::Enum(CalcitEnumValue {
        tag: Arc::new(Calcit::Tag(EdnTag::from("&"))),
        extra: vec![inner.to_calcit()],
        sum_type: None,
      }),
      Self::Custom(value) => value.as_ref().to_owned(),
      Self::Optional(inner) => Calcit::Enum(CalcitEnumValue {
        tag: Arc::new(Calcit::Tag(EdnTag::from("optional"))),
        extra: vec![inner.to_calcit()],
        sum_type: None,
      }),
      Self::JsNullish(inner) => Calcit::Enum(CalcitEnumValue {
        tag: Arc::new(Calcit::Tag(EdnTag::from("js-nullish"))),
        extra: vec![inner.to_calcit()],
        sum_type: None,
      }),
      Self::Struct(struct_def, args) => {
        if args.is_empty() {
          Calcit::StructDef((**struct_def).clone())
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
      Self::Enum(enum_def, _) => Calcit::EnumDef((**enum_def).clone()),
      Self::StructDef(struct_def) => Calcit::StructDef((**struct_def).clone()),
      Self::EnumDef(enum_def) => Calcit::EnumDef((**enum_def).clone()),
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
      Self::Dynamic => Edn::Symbol(Arc::from("Dynamic")),
      Self::Nil => Edn::Symbol(Arc::from("Nil")),
      Self::Unit => Edn::Symbol(Arc::from("Unit")),
      Self::Bool => Edn::Symbol(Arc::from("Bool")),
      Self::Number => Edn::Symbol(Arc::from("Number")),
      Self::String => Edn::Symbol(Arc::from("String")),
      Self::Symbol => Edn::Symbol(Arc::from("Symbol")),
      Self::Tag => Edn::Symbol(Arc::from("Tag")),
      Self::DynFn => Edn::Symbol(Arc::from("Fn")),
      Self::AnonymousEnum => Edn::Symbol(Arc::from("Enum")),
      Self::Buffer => Edn::Symbol(Arc::from("Buffer")),
      Self::F64Buffer => Edn::Symbol(Arc::from("F64Buffer")),
      Self::CirruQuote => Edn::Symbol(Arc::from("CirruQuote")),
      Self::JsObject => Edn::Symbol(Arc::from("JsObject")),
      // TypeVar: source syntax uses `'T`, while Cirru EDN stores that as `Edn::Symbol("T")`.
      Self::TypeVar(name) => Edn::Symbol(Arc::from(name.trim_start_matches('\''))),
      Self::TypeRef(name, args) => {
        if args.is_empty() {
          Edn::Symbol(Arc::from(name.trim_start_matches('\'')))
        } else {
          Edn::enum_value(name.trim_start_matches('\''), args.iter().map(|arg| arg.to_type_edn()).collect())
        }
      }
      // Bare containers share DYNAMIC_TYPE for their omitted arguments. An
      // explicitly written Dynamic is a distinct Arc and must round-trip.
      Self::List(inner) => {
        if Arc::ptr_eq(inner, &DYNAMIC_TYPE) {
          Edn::Symbol(Arc::from("List"))
        } else {
          Edn::enum_value("List", vec![inner.to_type_edn()])
        }
      }
      Self::Map(k, v) => {
        if Arc::ptr_eq(k, &DYNAMIC_TYPE) && Arc::ptr_eq(v, &DYNAMIC_TYPE) {
          Edn::Symbol(Arc::from("Map"))
        } else {
          Edn::enum_value("Map", vec![k.to_type_edn(), v.to_type_edn()])
        }
      }
      Self::Set(inner) => {
        if Arc::ptr_eq(inner, &DYNAMIC_TYPE) {
          Edn::Symbol(Arc::from("Set"))
        } else {
          Edn::enum_value("Set", vec![inner.to_type_edn()])
        }
      }
      Self::Ref(inner) => {
        if Arc::ptr_eq(inner, &DYNAMIC_TYPE) {
          Edn::Symbol(Arc::from("Ref"))
        } else {
          Edn::enum_value("Ref", vec![inner.to_type_edn()])
        }
      }
      Self::Optional(inner) => Edn::enum_value("Optional", vec![inner.to_type_edn()]),
      Self::JsNullish(inner) => Edn::enum_value("JsNullish", vec![inner.to_type_edn()]),
      Self::Variadic(inner) => Edn::enum_value("Variadic", vec![inner.to_type_edn()]),
      Self::Fn(fn_annot) => Edn::enum_value("Fn", vec![fn_annot.to_inline_type_schema_edn()]),
      Self::Struct(s, args) => {
        if args.is_empty() {
          Edn::Symbol(Arc::from(s.name.ref_str()))
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
      // Custom Calcit values – do a best-effort Calcit→Edn conversion. Older
      // in-memory `:any` annotations are always written in canonical form.
      Self::Custom(value) if Self::custom_keyword_matches(value, "any") => Edn::Symbol(Arc::from("Dynamic")),
      Self::Custom(value) if matches!(value.as_ref(), Calcit::Tag(tag) if Self::canonical_type_symbol_name(tag.ref_str()).is_some()) => {
        let Calcit::Tag(tag) = value.as_ref() else { unreachable!() };
        Edn::Symbol(Arc::from(
          Self::canonical_type_symbol_name(tag.ref_str()).expect("known canonical type symbol"),
        ))
      }
      Self::Custom(value) => calcit_type_to_edn(value.as_ref()),
      // Enum / Trait variants – use the name as a symbol
      Self::Enum(e, _) => Edn::Symbol(Arc::from(e.name().ref_str())),
      Self::StructDef(_) => Edn::Symbol(Arc::from("StructDef")),
      Self::EnumDef(_) => Edn::Symbol(Arc::from("EnumDef")),
      Self::StructValue(struct_def) => Edn::Symbol(Arc::from(struct_def.name.ref_str())),
      Self::EnumValue(enum_def) => Edn::Symbol(Arc::from(enum_def.name().ref_str())),
      Self::Trait(trait_def) => Edn::Symbol(Arc::from(trait_def.name.ref_str())),
      Self::TypeSlot(name) => Edn::Symbol(Arc::from(format!("*{name}"))),
      // Anything else falls back to dynamic
      _ => Edn::Symbol(Arc::from("Dynamic")),
    }
  }

  pub fn as_enum(&self) -> Option<&CalcitEnumDef> {
    match self {
      Self::EnumValue(enum_def) => Some(enum_def),
      Self::Enum(enum_def, _) => Some(enum_def),
      Self::Optional(inner) => inner.as_enum(),
      _ => None,
    }
  }

  pub fn as_struct(&self) -> Option<&CalcitStructDef> {
    match self {
      Self::StructValue(struct_def) => Some(struct_def),
      Self::Struct(struct_def, _) => Some(struct_def),
      Self::Custom(value) => match value.as_ref() {
        Calcit::Struct(struct_value) => Some(struct_value.struct_ref.as_ref()),
        Calcit::StructDef(struct_def) => Some(struct_def),
        _ => None,
      },
      Self::Optional(inner) => inner.as_struct(),
      _ => None,
    }
  }

  pub fn as_fn(&self) -> Option<&CalcitFnTypeAnnotation> {
    match self {
      Self::Fn(fn_annot) => Some(fn_annot.as_ref()),
      Self::Optional(inner) => inner.as_fn(),
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
    let mut remaining = 128;
    self.describe_bounded(0, &mut remaining)
  }

  fn describe_bounded(&self, depth: usize, remaining: &mut usize) -> String {
    if depth >= TYPE_DIAGNOSTIC_DEPTH_LIMIT || *remaining == 0 {
      return "…".to_owned();
    }
    *remaining -= 1;
    match self {
      Self::List(inner) => {
        if matches!(inner.as_ref(), Self::Dynamic) {
          return "list".to_string();
        }
        return format!("list<{}>", inner.describe_bounded(depth + 1, remaining));
      }
      Self::Map(k, v) => {
        if matches!(k.as_ref(), Self::Dynamic) && matches!(v.as_ref(), Self::Dynamic) {
          return "map".to_string();
        }
        return format!(
          "map<{}, {}>",
          k.describe_bounded(depth + 1, remaining),
          v.describe_bounded(depth + 1, remaining)
        );
      }
      Self::Set(inner) => {
        if matches!(inner.as_ref(), Self::Dynamic) {
          return "set".to_string();
        }
        return format!("set<{}>", inner.describe_bounded(depth + 1, remaining));
      }
      Self::Ref(inner) => {
        if matches!(inner.as_ref(), Self::Dynamic) {
          return "ref".to_string();
        }
        return format!("ref<{}>", inner.describe_bounded(depth + 1, remaining));
      }
      _ => {}
    }

    if let Some(tag) = self.builtin_tag_name() {
      return tag.to_string();
    }

    match self {
      Self::Fn(signature) => signature.describe_bounded(depth + 1, remaining),
      Self::Macro(_) => "macro-signature".to_owned(),
      Self::Syntax(contract) => match contract.as_ref() {
        MacroSyntaxType::Syntax => "syntax".to_owned(),
        MacroSyntaxType::SyntaxSymbol => "syntax-symbol".to_owned(),
        MacroSyntaxType::SyntaxList => "syntax-list".to_owned(),
        MacroSyntaxType::Expr(semantic) => format!("syntax-expr<{}>", semantic.describe_bounded(depth + 1, remaining)),
      },
      Self::Variadic(inner) => format!("variadic {}", inner.describe_bounded(depth + 1, remaining)),
      Self::Custom(_) => "custom".to_string(),
      Self::Optional(inner) => format!("optional<{}>", inner.describe_bounded(depth + 1, remaining)),
      Self::JsNullish(inner) => format!("js-nullish<{}>", inner.describe_bounded(depth + 1, remaining)),
      Self::Struct(base, args) => {
        if args.is_empty() {
          format!("struct {}", base.name)
        } else {
          let rendered = render_bounded_type_list(args, depth + 1, remaining, CalcitTypeAnnotation::describe_bounded);
          format!("struct {}<{}>", base.name, rendered)
        }
      }
      Self::TypeVar(name) => format!("'{name}"),
      Self::TypeRef(name, args) => {
        if args.is_empty() {
          format!("type {name}")
        } else {
          let rendered = render_bounded_type_list(args, depth + 1, remaining, CalcitTypeAnnotation::describe_bounded);
          format!("type {name}<{rendered}>")
        }
      }
      Self::Enum(enum_def, args) => {
        if args.is_empty() {
          format!("enum {}", enum_def.name())
        } else {
          let rendered = render_bounded_type_list(args, depth + 1, remaining, CalcitTypeAnnotation::describe_bounded);
          format!("enum {}<{}>", enum_def.name(), rendered)
        }
      }
      Self::StructValue(struct_def) => format!("struct {}", struct_def.name),
      Self::EnumValue(enum_def) => format!("enum {}", enum_def.name()),
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
      Self::F64Buffer => 11,
      Self::CirruQuote => 11,
      Self::StructValue(_) => 12,
      Self::EnumValue(_) => 13,
      Self::AnonymousEnum => 14,
      Self::Fn(_) => 15,
      Self::Set(_) => 16,
      Self::Variadic(_) => 17,
      Self::Custom(_) => 18,
      Self::Optional(_) => 19,
      Self::JsNullish(_) => 20,
      Self::Dynamic => 21,
      Self::TypeVar(_) => 22,
      Self::TypeRef(_, _) => 23,
      Self::Struct(_, _) => 24,
      Self::Enum(_, _) => 25,
      Self::Trait(_) => 26,
      Self::TraitSet(_) => 27,
      Self::Nil => 28,
      Self::Unit => 29,
      Self::JsObject => 30,
      Self::TypeSlot(_) => 31,
      Self::StructDef(_) => 32,
      Self::EnumDef(_) => 33,
      Self::Macro(_) => 34,
      Self::Syntax(_) => 35,
    }
  }
}

fn resolve_struct_annotation(struct_form: &Calcit, class_form: Option<&Calcit>) -> Option<CalcitStructDef> {
  let mut struct_def = resolve_struct_def(struct_form)?;
  if let Some(class_struct) = class_form.and_then(resolve_struct_value) {
    struct_def.impls = vec![Arc::new(CalcitImpl::from_struct(&class_struct))];
  }
  Some(struct_def)
}

fn split_nominal_definition_ref(definition_ref: Option<&str>) -> Option<(Arc<str>, Arc<str>)> {
  definition_ref
    .and_then(|path| path.rsplit_once('/'))
    .map(|(namespace, definition)| (Arc::from(namespace), Arc::from(definition)))
}

fn resolve_enum_annotation(enum_form: &Calcit, class_form: Option<&Calcit>) -> Option<CalcitEnumDef> {
  let mut enum_def = resolve_enum_def(enum_form)?;
  if let Some(class_struct) = class_form.and_then(resolve_struct_value) {
    enum_def.set_impls(vec![Arc::new(CalcitImpl::from_struct(&class_struct))]);
  }
  Some(enum_def)
}

fn resolve_struct_def(form: &Calcit) -> Option<CalcitStructDef> {
  match form {
    Calcit::StructDef(struct_def) => Some(struct_def.to_owned()),
    Calcit::Struct(struct_value) => Some(struct_value.struct_ref.as_ref().to_owned()),
    _ => resolve_calcit_value(form).and_then(|value| match value {
      Calcit::StructDef(struct_def) => Some(struct_def),
      Calcit::Struct(struct_value) => Some(struct_value.struct_ref.as_ref().to_owned()),
      _ => None,
    }),
  }
}

/// Resolve a struct definition by namespace and definition name from the program registry.
/// Used by `CalcitTypeAnnotation::resolve_to_struct` to look up `TypeRef("ns/def")` at compile time.
fn resolve_struct_from_program(ns: &str, def: &str) -> Option<CalcitStructDef> {
  lookup_runtime_ready_registered(ns, def)
    .and_then(|value| match &value {
      Calcit::StructDef(s) => Some(s.to_owned()),
      _ => resolve_type_def_from_code(&value).and_then(|resolved| match resolved {
        Calcit::StructDef(s) => Some(s),
        _ => None,
      }),
    })
    .or_else(|| {
      lookup_def_code_registered(ns, def).and_then(|code| {
        resolve_type_def_from_code(&code).and_then(|resolved| match resolved {
          Calcit::StructDef(s) => Some(s),
          _ => None,
        })
      })
    })
    .map(|struct_def| {
      if struct_def.definition_ref.is_some() {
        struct_def
      } else {
        struct_def.with_definition_ref(ns, def)
      }
    })
}

/// Resolve an enum definition by namespace and definition name from the program registry.
/// Used by `CalcitTypeAnnotation::resolve_to_enum` to look up `TypeRef("ns/def")` at compile time.
fn resolve_enum_from_program(ns: &str, def: &str) -> Option<CalcitEnumDef> {
  lookup_runtime_ready_registered(ns, def)
    .and_then(|value| match &value {
      Calcit::EnumDef(e) => Some(e.to_owned()),
      Calcit::Struct(struct_value) => CalcitEnumDef::from_struct(struct_value.to_owned()).ok(),
      _ => resolve_type_def_from_code(&value).and_then(|resolved| match resolved {
        Calcit::EnumDef(e) => Some(e),
        Calcit::Struct(struct_value) => CalcitEnumDef::from_struct(struct_value).ok(),
        _ => None,
      }),
    })
    .or_else(|| {
      lookup_def_code_registered(ns, def).and_then(|code| {
        resolve_type_def_from_code(&code).and_then(|resolved| match resolved {
          Calcit::EnumDef(e) => Some(e),
          Calcit::Struct(struct_value) => CalcitEnumDef::from_struct(struct_value).ok(),
          _ => None,
        })
      })
    })
    .map(|enum_def| {
      if enum_def.definition_ref().is_some() {
        enum_def
      } else {
        enum_def.with_definition_ref(ns, def)
      }
    })
}

fn resolve_enum_def(form: &Calcit) -> Option<CalcitEnumDef> {
  match form {
    Calcit::EnumDef(enum_def) => Some(enum_def.to_owned()),
    Calcit::Struct(struct_value) => CalcitEnumDef::from_struct(struct_value.to_owned()).ok(),
    _ => resolve_calcit_value(form).and_then(|value| match value {
      Calcit::EnumDef(enum_def) => Some(enum_def),
      Calcit::Struct(struct_value) => CalcitEnumDef::from_struct(struct_value).ok(),
      _ => None,
    }),
  }
}

fn resolve_struct_value(form: &Calcit) -> Option<CalcitStructValue> {
  match form {
    Calcit::Struct(struct_value) => Some(struct_value.to_owned()),
    _ => resolve_calcit_value(form).and_then(|value| match value {
      Calcit::Struct(struct_value) => Some(struct_value),
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
    Calcit::Enum(enum_value) => Edn::enum_value(
      match enum_value.tag.as_ref() {
        Calcit::Tag(tag) => tag.ref_str(),
        Calcit::Symbol { sym, .. } => sym.as_ref(),
        _ => return Edn::Nil,
      },
      enum_value.extra.iter().map(calcit_type_to_edn).collect(),
    ),
    _ => Edn::Nil,
  }
}

/// Check whether a definition's source code form resolves to a concrete
/// StructDef or EnumDef. Used to restrict direct `assert-type` type resolution
/// to nominal type definitions, so visible function or value names are never
/// parsed as resolved types.
pub(crate) fn code_resolves_to_nominal_type_def(code: &Calcit) -> bool {
  matches!(
    resolve_type_def_from_code(code),
    Some(Calcit::StructDef(_)) | Some(Calcit::EnumDef(_))
  )
}

pub(crate) fn resolve_type_def_from_code(code: &Calcit) -> Option<Calcit> {
  // Unwrap thunks: defstruct/defenum definitions are stored as unevaluated thunks
  if let Calcit::Thunk(thunk) = code {
    return resolve_type_def_from_code(thunk.get_code());
  }
  let Calcit::List(items) = code else {
    return None;
  };
  if let Some(head) = items.first()
    && (matches!(head, Calcit::Syntax(CalcitSyntax::Quote, _))
      || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "quote")
      || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "quote"))
    && let Some(inner) = items.get(1)
  {
    return resolve_type_def_from_code(inner);
  }
  let head = items.first()?;
  // Data definitions are often stored behind `def` and `impl-traits` wrappers.
  // Peel those value-preserving forms before looking for defstruct/defenum so
  // named type references retain their concrete runtime representation.
  if is_def_head(head)
    && let Some(value) = items.get(2)
  {
    return resolve_type_def_from_code(value);
  }
  if is_impl_traits_head(head)
    && let Some(value) = items.get(1)
  {
    return resolve_type_def_from_code(value);
  }
  if is_defstruct_head(head) || is_struct_new_head(head) {
    return parse_defstruct_code(items.as_ref()).map(Calcit::StructDef);
  }
  if is_defenum_head(head) || is_enum_new_head(head) {
    return parse_defenum_code(items.as_ref()).map(Calcit::EnumDef);
  }
  None
}

fn is_def_head(head: &Calcit) -> bool {
  matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "def")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "def")
}

fn is_impl_traits_head(head: &Calcit) -> bool {
  matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "impl-traits")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "impl-traits")
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
    || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "&struct-def:new")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "&struct-def:new")
}

fn is_enum_new_head(head: &Calcit) -> bool {
  matches!(head, Calcit::Proc(CalcitProc::NativeEnumNew))
    || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "&enum-def:new")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "&enum-def:new")
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

/// Generic arguments are positional: deduplicate without changing declaration order.
pub(crate) fn dedup_generic_names(generics: &mut Vec<Arc<str>>) {
  let mut seen = HashSet::with_capacity(generics.len());
  generics.retain(|name| seen.insert(name.clone()));
}

fn parse_defstruct_code(items: &CalcitList) -> Option<CalcitStructDef> {
  let forms = normalized_data_definition_forms(items);
  let name_form = forms.get(1)?;
  let name = parse_type_name(name_form)?;
  let definition_ref = match name_form {
    Calcit::Symbol { info, .. } => Some(Arc::from(format!("{}/{}", info.at_ns, name.ref_str()))),
    _ => None,
  };
  let mut generics: Vec<Arc<str>> = vec![];
  let mut where_bounds = vec![];
  let mut start_idx = 2;

  if let Some(generics_form) = forms.get(2)
    && let Some(vars) = CalcitTypeAnnotation::parse_generics_list(generics_form)
  {
    generics = vars;
    start_idx = 3;
  }
  let has_where_form = forms.get(start_idx).is_some_and(|form| match form {
    Calcit::Map(_) => true,
    Calcit::List(xs) => xs.first().is_some_and(CalcitTypeAnnotation::is_schema_map_literal_head),
    _ => false,
  });
  if has_where_form {
    let form = forms.get(start_idx)?;
    where_bounds = CalcitTypeAnnotation::parse_where_bounds_form(form, generics.as_slice(), true);
    start_idx += 1;
  }
  let mut fields: Vec<(EdnTag, Arc<CalcitTypeAnnotation>)> = Vec::new();

  for item in forms.iter().skip(start_idx) {
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

  dedup_generic_names(&mut generics);

  let field_names: Vec<EdnTag> = fields.iter().map(|(name, _)| name.to_owned()).collect();
  let field_types: Vec<Arc<CalcitTypeAnnotation>> = fields.iter().map(|(_, t)| t.to_owned()).collect();

  Some(CalcitStructDef {
    definition_ref,
    name,
    fields: Arc::new(field_names),
    field_types: Arc::new(field_types),
    generics: Arc::new(generics),
    where_bounds: Arc::new(where_bounds),
    impls: vec![],
  })
}

fn parse_defenum_code(items: &CalcitList) -> Option<CalcitEnumDef> {
  let forms = normalized_data_definition_forms(items);
  let name_form = forms.get(1)?;
  let name = parse_type_name(name_form)?;
  let definition_ref = match name_form {
    Calcit::Symbol { info, .. } => Some(Arc::from(format!("{}/{}", info.at_ns, name.ref_str()))),
    _ => None,
  };
  let mut generics: Vec<Arc<str>> = vec![];
  let mut where_bounds = vec![];
  let mut start_idx = 2;

  if let Some(generics_form) = forms.get(2)
    && let Some(vars) = CalcitTypeAnnotation::parse_generics_list(generics_form)
  {
    generics = vars;
    start_idx = 3;
  }
  let has_where_form = forms.get(start_idx).is_some_and(|form| match form {
    Calcit::Map(_) => true,
    Calcit::List(xs) => xs.first().is_some_and(CalcitTypeAnnotation::is_schema_map_literal_head),
    _ => false,
  });
  if has_where_form {
    let form = forms.get(start_idx)?;
    where_bounds = CalcitTypeAnnotation::parse_where_bounds_form(form, generics.as_slice(), true);
    start_idx += 1;
  }

  let mut variants: Vec<(EdnTag, Calcit)> = Vec::new();
  for item in forms.iter().skip(start_idx) {
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
  dedup_generic_names(&mut generics);
  let mut struct_ref = CalcitStructDef::from_fields(name, fields);
  struct_ref.definition_ref = definition_ref;
  struct_ref.generics = Arc::new(generics);
  struct_ref.where_bounds = Arc::new(where_bounds);
  let struct_value = CalcitStructValue {
    struct_ref: Arc::new(struct_ref),
    values: Arc::new(values),
  };
  CalcitEnumDef::from_struct(struct_value).ok()
}

/// `defstruct` and `defenum` accept a map-headed wrapper (`$ {} ...`) so data
/// definitions can be supplied as one macro argument. Runtime macros normalize
/// that form before parsing generics and `:where`; do the same for static type
/// resolution so a named TypeRef observes identical fields or variants.
fn normalized_data_definition_forms(items: &CalcitList) -> Vec<&Calcit> {
  let Some(Calcit::List(wrapper)) = items.get(2) else {
    return items.iter().collect();
  };
  if items.len() != 3 || !wrapper.first().is_some_and(CalcitTypeAnnotation::is_schema_map_literal_head) {
    return items.iter().collect();
  }
  let mut forms = vec![items.first().expect("definition head"), items.get(1).expect("definition name")];
  forms.extend(wrapper.iter().skip(1));
  forms
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
  use std::collections::BTreeSet;

  fn nested_relation_fixture(depth: usize, leaf: CalcitTypeAnnotation) -> Arc<CalcitTypeAnnotation> {
    let mut current = Arc::new(leaf);
    for level in 0..depth {
      current = Arc::new(match level % 4 {
        0 => CalcitTypeAnnotation::List(current),
        1 => CalcitTypeAnnotation::Optional(current),
        2 => CalcitTypeAnnotation::Map(Arc::new(CalcitTypeAnnotation::String), current),
        _ => CalcitTypeAnnotation::TypeRef(Arc::from("tests.types/Layer"), Arc::new(vec![current])),
      });
    }
    current
  }

  #[test]
  fn bounded_plain_relations_keep_deep_types_precise_without_recursion() {
    for depth in [32, 256, 2_048] {
      let actual = nested_relation_fixture(depth, CalcitTypeAnnotation::Number);
      let expected = nested_relation_fixture(depth, CalcitTypeAnnotation::Number);
      let mismatch = nested_relation_fixture(depth, CalcitTypeAnnotation::String);

      let cold_started = std::time::Instant::now();
      let (proof, stats) = bounded_plain_type_relation(actual.as_ref(), expected.as_ref(), PlainRelationMode::Proof)
        .expect("fixture is inside the relation budget")
        .expect("fixture uses the iterative relation subset");
      let cold_elapsed = cold_started.elapsed();
      let warm_started = std::time::Instant::now();
      let (_, warm_stats) = bounded_plain_type_relation(actual.as_ref(), expected.as_ref(), PlainRelationMode::Proof)
        .expect("repeated relation is inside the relation budget")
        .expect("repeated fixture uses the iterative relation subset");
      let warm_elapsed = warm_started.elapsed();
      assert_eq!(proof, TypeProof::Proven);
      assert!(stats.node_visits > depth, "depth {depth}: {stats:?}");
      assert!(stats.max_depth >= depth, "depth {depth}: {stats:?}");
      assert_eq!(stats.node_visits, warm_stats.node_visits, "relation memo must remain context-local");
      let tracked_payload_bytes = stats.node_visits * std::mem::size_of::<(usize, usize)>()
        + stats.max_worklist * std::mem::size_of::<(&CalcitTypeAnnotation, &CalcitTypeAnnotation, usize)>();
      eprintln!(
        "type-relation depth={depth} visits={} memo-hits={} max-worklist={} tracked-payload-bytes={} allocator-peak=unavailable cold-us={} warm-us={}",
        stats.node_visits,
        stats.memo_hits,
        stats.max_worklist,
        tracked_payload_bytes,
        cold_elapsed.as_micros(),
        warm_elapsed.as_micros()
      );
      assert!(actual.is_proven_for(expected.as_ref()));
      assert!(!actual.is_proven_for(mismatch.as_ref()));
    }
  }

  #[test]
  fn bounded_plain_relations_memoize_shared_type_dags() {
    let actual_shared = nested_relation_fixture(64, CalcitTypeAnnotation::Number);
    let expected_shared = nested_relation_fixture(64, CalcitTypeAnnotation::Number);
    let actual = CalcitTypeAnnotation::Map(actual_shared.clone(), actual_shared);
    let expected = CalcitTypeAnnotation::Map(expected_shared.clone(), expected_shared);

    let (proof, stats) = bounded_plain_type_relation(&actual, &expected, PlainRelationMode::Proof)
      .expect("DAG is inside the relation budget")
      .expect("DAG uses the iterative relation subset");
    assert_eq!(proof, TypeProof::Proven);
    assert!(stats.memo_hits >= 1, "shared pair should be visited once: {stats:?}");
    assert!(
      stats.node_visits < 100,
      "shared subtree should not be copied or revisited: {stats:?}"
    );
  }

  #[test]
  fn type_variable_substitution_reuses_shared_dag_results_per_call() {
    let shared = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("tests.shared/Box"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))]),
    ));
    let annotation = CalcitTypeAnnotation::Map(shared.clone(), shared);
    let substituted = annotation.substitute_type_vars(&HashMap::from([(Arc::from("T"), Arc::new(CalcitTypeAnnotation::Number))]));
    let CalcitTypeAnnotation::Map(key, value) = substituted.as_ref() else {
      panic!("map substitution must preserve the container")
    };
    assert!(Arc::ptr_eq(key, value), "one substitution context must preserve DAG sharing");

    let next = annotation.substitute_type_vars(&HashMap::from([(Arc::from("T"), Arc::new(CalcitTypeAnnotation::String))]));
    assert!(!Arc::ptr_eq(&substituted, &next), "a later context must not reuse stale results");
    let CalcitTypeAnnotation::Map(key, _) = next.as_ref() else {
      panic!("map substitution must preserve the container")
    };
    let CalcitTypeAnnotation::TypeRef(_, args) = key.as_ref() else {
      panic!("map key must preserve its nominal reference")
    };
    assert_eq!(args.as_slice(), &[Arc::new(CalcitTypeAnnotation::String)]);
  }

  #[test]
  fn data_schema_identity_uses_the_same_bounded_deep_traversal() {
    let actual = nested_relation_fixture(2_048, CalcitTypeAnnotation::Number);
    let expected = nested_relation_fixture(2_048, CalcitTypeAnnotation::Number);
    let mismatch = nested_relation_fixture(2_048, CalcitTypeAnnotation::String);
    assert!(same_data_schema_annotation(actual.as_ref(), expected.as_ref()));
    assert!(!same_data_schema_annotation(actual.as_ref(), mismatch.as_ref()));
  }

  #[test]
  fn mutually_recursive_nominal_refs_remain_finite_and_preserve_payloads() {
    let type_var = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let tree_ref = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("tests.recursive/Tree"),
      Arc::new(vec![type_var.clone()]),
    ));
    let node_ref = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("tests.recursive/Node"),
      Arc::new(vec![type_var.clone()]),
    ));
    let tree = CalcitStructDef {
      definition_ref: Some(Arc::from("tests.recursive/Tree")),
      name: EdnTag::new("Tree"),
      fields: Arc::new(vec![EdnTag::new("children")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::List(node_ref))]),
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    };
    let node = CalcitStructDef {
      definition_ref: Some(Arc::from("tests.recursive/Node")),
      name: EdnTag::new("Node"),
      fields: Arc::new(vec![EdnTag::new("value"), EdnTag::new("tree")]),
      field_types: Arc::new(vec![type_var.clone(), tree_ref.clone()]),
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    };
    assert!(same_data_schema_annotation(
      &CalcitTypeAnnotation::Struct(Arc::new(tree.clone()), Arc::new(vec![type_var.clone()])),
      &CalcitTypeAnnotation::Struct(Arc::new(tree), Arc::new(vec![type_var.clone()]))
    ));
    assert!(same_data_schema_annotation(
      &CalcitTypeAnnotation::Struct(Arc::new(node.clone()), Arc::new(vec![type_var.clone()])),
      &CalcitTypeAnnotation::Struct(Arc::new(node), Arc::new(vec![type_var.clone()]))
    ));

    let substituted = tree_ref.substitute_type_vars(&HashMap::from([(Arc::from("T"), Arc::new(CalcitTypeAnnotation::Number))]));
    let expected = CalcitTypeAnnotation::TypeRef(
      Arc::from("tests.recursive/Tree"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
    );
    assert!(substituted.is_proven_for(&expected), "recursive payload T must remain Number");
  }

  #[test]
  fn bounded_plain_relations_report_complexity_instead_of_dynamic_fallback() {
    let actual = nested_relation_fixture(TYPE_RELATION_NODE_LIMIT + 32, CalcitTypeAnnotation::Number);
    let expected = nested_relation_fixture(TYPE_RELATION_NODE_LIMIT + 32, CalcitTypeAnnotation::Number);
    assert_eq!(
      actual.prove_with_bindings(expected.as_ref(), &mut TypeBindings::new()),
      TypeProof::NeedsBoundary(TypeBoundaryReason::TypeComplexityLimit)
    );
    assert!(!actual.is_compatible_with(expected.as_ref()));

    // Avoid recursively dropping a deliberately over-budget test fixture.
    std::mem::forget(actual);
    std::mem::forget(expected);
  }

  #[test]
  fn deep_type_diagnostics_are_truncated_before_recursive_rendering_grows() {
    let mut annotation = Arc::new(CalcitTypeAnnotation::Number);
    for _ in 0..2_048 {
      annotation = Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from("tests.types/Layer"),
        Arc::new(vec![annotation]),
      ));
    }
    let brief = annotation.to_brief_string();
    let described = annotation.describe();
    assert!(brief.contains('…'), "brief diagnostic should expose truncation: {brief}");
    assert!(
      described.contains('…'),
      "described diagnostic should expose truncation: {described}"
    );
    assert!(brief.len() < 1_024, "brief diagnostic is unexpectedly large: {} bytes", brief.len());
    assert!(
      described.len() < 1_024,
      "described diagnostic is unexpectedly large: {} bytes",
      described.len()
    );

    let signature = CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![annotation.clone(); 1_024],
      return_type: annotation,
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    }));
    let fn_brief = signature.to_brief_string();
    let fn_description = signature.describe();
    assert!(
      fn_brief.contains('…') && fn_brief.len() < 8_192,
      "function brief must share the render budget"
    );
    assert!(
      fn_description.contains('…') && fn_description.len() < 8_192,
      "function description must share the render budget"
    );
  }

  #[test]
  fn compatibility_budget_spans_nested_function_signature_relations() {
    let width = TYPE_RELATION_NODE_LIMIT + 1;
    let signature = |arg_types| {
      CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types,
        return_type: Arc::new(CalcitTypeAnnotation::Unit),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      }))
    };
    let actual = signature((0..width).map(|_| Arc::new(CalcitTypeAnnotation::Number)).collect());
    let expected = signature((0..width).map(|_| Arc::new(CalcitTypeAnnotation::Number)).collect());
    assert!(!actual.is_compatible_with(&expected));
  }

  #[test]
  fn pure_schema_alias_cycles_are_not_compatible_or_proven() {
    use crate::program::{PROGRAM_CODE_DATA, ProgramDefEntry, ProgramFileData, lock_program_test_state};

    let _guard = lock_program_test_state();
    register_program_lookups(
      crate::program::lookup_runtime_ready,
      crate::program::lookup_def_code,
      crate::program::lookup_def_schema,
    );
    let alias_ref = |name: &'static str| {
      Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from(format!("tests.alias-cycle/{name}")),
        Arc::new(vec![]),
      ))
    };
    PROGRAM_CODE_DATA.write().expect("seed alias cycle").insert(
      Arc::from("tests.alias-cycle"),
      ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([
          (
            Arc::from("A"),
            ProgramDefEntry {
              code: Calcit::Nil,
              schema: alias_ref("B"),
              doc: Arc::from(""),
              examples: vec![],
              ffi: None,
            },
          ),
          (
            Arc::from("B"),
            ProgramDefEntry {
              code: Calcit::Nil,
              schema: alias_ref("A"),
              doc: Arc::from(""),
              examples: vec![],
              ffi: None,
            },
          ),
        ]),
      },
    );

    let alias = alias_ref("A");
    assert!(!alias.is_compatible_with(&CalcitTypeAnnotation::Number));
    assert_eq!(
      alias.prove_with_bindings(&CalcitTypeAnnotation::Number, &mut TypeBindings::new()),
      TypeProof::NeedsBoundary(TypeBoundaryReason::RecursiveTypeAlias)
    );
  }

  #[test]
  fn runtime_value_validation_expands_schema_aliases_and_rejects_cycles() {
    use crate::program::{PROGRAM_CODE_DATA, ProgramDefEntry, ProgramFileData, lock_program_test_state};

    let _guard = lock_program_test_state();
    register_program_lookups(
      crate::program::lookup_runtime_ready,
      crate::program::lookup_def_code,
      crate::program::lookup_def_schema,
    );
    let namespace = "tests.runtime-alias";
    let alias_ref = |name: &'static str| {
      Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from(format!("{namespace}/{name}")),
        Arc::new(vec![]),
      ))
    };
    let display_trait = Arc::new(CalcitTrait::new(EdnTag::new("Display"), vec![], vec![]).with_definition_ref(namespace, "Display"));
    let entry = |schema| ProgramDefEntry {
      code: Calcit::Nil,
      schema,
      doc: Arc::from(""),
      examples: vec![],
      ffi: None,
    };
    PROGRAM_CODE_DATA.write().expect("seed runtime aliases").insert(
      Arc::from(namespace),
      ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([
          (
            Arc::from("Items"),
            entry(Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Number)))),
          ),
          (
            Arc::from("Callback"),
            entry(Arc::new(CalcitTypeAnnotation::from_function_parts(
              vec![Arc::new(CalcitTypeAnnotation::Number)],
              Arc::new(CalcitTypeAnnotation::Number),
            ))),
          ),
          (
            Arc::from("Displayable"),
            entry(Arc::new(CalcitTypeAnnotation::Trait(display_trait.clone()))),
          ),
          (Arc::from("A"), entry(alias_ref("B"))),
          (Arc::from("B"), entry(alias_ref("A"))),
        ]),
      },
    );

    assert!(value_matches_type_annotation(
      &Calcit::List(Arc::new(CalcitList::default())),
      alias_ref("Items").as_ref()
    ));
    assert!(!value_matches_type_annotation(
      &Calcit::List(Arc::new(CalcitList::default())),
      &CalcitTypeAnnotation::TypeRef(
        Arc::from(format!("{namespace}/Items")),
        Arc::new(vec![Arc::new(CalcitTypeAnnotation::String)]),
      )
    ));
    assert!(value_matches_type_annotation(
      &Calcit::Proc(CalcitProc::List),
      alias_ref("Callback").as_ref()
    ));

    let widget = CalcitStructDef {
      definition_ref: Some(Arc::from(format!("{namespace}/Widget"))),
      name: EdnTag::new("Widget"),
      fields: Arc::new(vec![]),
      field_types: Arc::new(vec![]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![Arc::new(CalcitImpl {
        name: EdnTag::new("WidgetDisplay"),
        origin: Some(display_trait),
        fields: Arc::new(vec![]),
        values: Arc::new(vec![]),
      })],
    };
    let widget_value = Calcit::Struct(CalcitStructValue {
      struct_ref: Arc::new(widget),
      values: Arc::new(vec![]),
    });
    assert!(value_matches_type_annotation(&widget_value, alias_ref("Displayable").as_ref()));
    assert!(!value_matches_type_annotation(&Calcit::Number(1.0), alias_ref("A").as_ref()));
  }

  #[test]
  fn phase_aware_macro_signature_round_trips_without_fn_conflation() {
    let mut map = EdnMapView::default();
    map.insert_key("generics", Edn::List(EdnListView(vec![Edn::Symbol(Arc::from("T"))])));
    map.insert_key(
      "required",
      Edn::List(EdnListView(vec![
        Edn::Symbol(Arc::from("SyntaxSymbol")),
        Edn::enum_value("Expr", vec![Edn::Symbol(Arc::from("T"))]),
      ])),
    );
    map.insert_key("optional", Edn::List(EdnListView(vec![Edn::Symbol(Arc::from("SyntaxList"))])));
    map.insert_key("rest", Edn::Symbol(Arc::from("Syntax")));
    map.insert_key("expansion", Edn::enum_value("Expr", vec![Edn::Symbol(Arc::from("T"))]));
    map.insert_key(
      "capabilities",
      Edn::Set(EdnSetView(HashSet::from([
        Edn::tag("env-read"),
        Edn::tag("fs-read"),
        Edn::tag("log"),
      ]))),
    );
    let schema = Edn::enum_value("Macro", vec![Edn::Map(map)]);

    let signature = CalcitTypeAnnotation::parse_macro_signature_from_edn(&schema).expect("strict macro signature");
    assert!(
      CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema).is_none(),
      "strict macro contracts must not be accepted as runtime function annotations"
    );
    assert!(signature.is_strict());
    assert!(matches!(
      signature.required_inputs.as_slice(),
      [MacroSyntaxType::SyntaxSymbol, MacroSyntaxType::Expr(_)]
    ));
    assert!(matches!(signature.optional_inputs.as_slice(), [MacroSyntaxType::SyntaxList]));
    assert!(matches!(signature.rest_input, Some(MacroSyntaxType::Syntax)));
    assert!(matches!(signature.expansion, MacroExpansionType::Expr(_)));
    assert!(signature.capabilities.contains(&MacroCapability::EnvRead));
    assert!(signature.capabilities.contains(&MacroCapability::FsRead));
    assert!(signature.capabilities.contains(&MacroCapability::Log));
    assert!(!signature.is_cache_eligible());

    let reloaded = CalcitTypeAnnotation::parse_macro_signature_from_edn(&signature.to_wrapped_schema_edn()).expect("round trip");
    assert_eq!(reloaded, signature);
  }

  #[test]
  fn legacy_macro_schema_is_not_parsed_as_a_macro_contract() {
    let mut map = EdnMapView::default();
    map.insert_key("args", Edn::List(EdnListView(vec![Edn::Symbol(Arc::from("Struct"))])));
    map.insert_key("return", Edn::Symbol(Arc::from("Struct")));
    let schema = Edn::enum_value("Macro", vec![Edn::Map(map)]);
    assert!(
      CalcitTypeAnnotation::parse_macro_signature_from_edn(&schema).is_none(),
      "runtime :args/:return schemas must not be accepted as Macro contracts"
    );
  }

  fn core_impl_trait_names(definition: &str) -> BTreeSet<String> {
    fn visit(node: &cirru_parser::Cirru, names: &mut BTreeSet<String>) {
      let cirru_parser::Cirru::List(items) = node else {
        return;
      };
      if let (Some(cirru_parser::Cirru::Leaf(head)), Some(cirru_parser::Cirru::Leaf(trait_name))) = (items.first(), items.get(1))
        && head.as_ref() == "&impl::new"
      {
        names.insert(trait_name.to_string());
      }
      for item in items {
        visit(item, names);
      }
    }

    let core = crate::load_core_snapshot().expect("load embedded core snapshot");
    let entry = core
      .files
      .get(CORE_NS)
      .and_then(|file| file.defs.get(definition))
      .unwrap_or_else(|| panic!("missing {definition} in embedded core snapshot"));
    let mut names = BTreeSet::new();
    visit(&entry.code, &mut names);
    names
  }

  #[test]
  fn bootstrap_core_trait_names_match_core_impl_definitions() {
    let dynamic = crate::calcit::DYNAMIC_TYPE.clone();
    let cases = vec![
      (CalcitTypeAnnotation::List(dynamic.clone()), "&core-list-impls"),
      (CalcitTypeAnnotation::Map(dynamic.clone(), dynamic.clone()), "&core-map-impls"),
      (CalcitTypeAnnotation::Set(dynamic.clone()), "&core-set-impls"),
      (CalcitTypeAnnotation::Ref(dynamic.clone()), "&core-ref-impls"),
      (CalcitTypeAnnotation::String, "&core-string-impls"),
      (CalcitTypeAnnotation::Number, "&core-number-impls"),
      (CalcitTypeAnnotation::DynFn, "&core-fn-impls"),
      (CalcitTypeAnnotation::Bool, "&core-scalar-impls"),
    ];

    for (annotation, definition) in cases {
      assert_eq!(
        annotation.core_impl_list_symbol(),
        Some(definition),
        "implementation mapping drifted for {definition}"
      );
      let expected: BTreeSet<String> = annotation
        .builtin_core_trait_names()
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
      assert_eq!(
        core_impl_trait_names(definition),
        expected,
        "bootstrap metadata drifted for {definition}"
      );
    }
  }

  #[test]
  fn bootstrap_core_trait_fallback_rejects_non_core_same_name() {
    let number = CalcitTypeAnnotation::Number;
    let core_debug = Arc::new(CalcitTrait::new_reference("calcit.core/Debug"));
    let core_show = Arc::new(CalcitTrait::new_reference("calcit.core/Show"));
    let user_debug = Arc::new(CalcitTrait::new_reference("app.main/Debug"));
    let runtime_user_debug = Arc::new(CalcitTrait::new_runtime(EdnTag::new("Debug"), vec![], vec![]));

    assert!(number.is_compatible_with(&CalcitTypeAnnotation::Trait(core_debug)));
    assert!(!number.is_compatible_with(&CalcitTypeAnnotation::Trait(core_show)));
    assert!(!number.is_compatible_with(&CalcitTypeAnnotation::Trait(user_debug)));
    assert!(!number.is_compatible_with(&CalcitTypeAnnotation::Trait(runtime_user_debug)));
  }

  #[test]
  fn nominal_trait_annotations_match_their_type_refs_and_own_bounds() {
    let dom_element = Arc::new(CalcitTrait::new_reference("respo.dom/DomElement"));
    let other_dom_element = Arc::new(CalcitTrait::new_reference("other.dom/DomElement"));
    let mut evaluated_dom_element = CalcitTrait::new_reference("respo.dom/DomElement");
    evaluated_dom_element.runtime_id = Some(7);
    let annotation = CalcitTypeAnnotation::Trait(Arc::new(evaluated_dom_element));

    assert!(annotation.is_compatible_with(&CalcitTypeAnnotation::TypeRef(Arc::from("respo.dom/DomElement"), Arc::new(vec![]),)));
    assert!(!annotation.is_compatible_with(&CalcitTypeAnnotation::TypeRef(Arc::from("other.dom/DomElement"), Arc::new(vec![]),)));
    assert!(annotation.satisfies_trait_bound(dom_element.as_ref()));
    assert!(!annotation.satisfies_trait_bound(other_dom_element.as_ref()));
    let CalcitTypeAnnotation::Trait(evaluated_dom_element) = &annotation else {
      unreachable!();
    };
    assert!(CalcitTypeAnnotation::Trait(dom_element).satisfies_trait_bound(evaluated_dom_element));
  }

  #[test]
  fn transitive_trait_requirements_prove_bounds_but_invalid_schemas_do_not() {
    let required = Arc::new(
      CalcitTrait::new(
        EdnTag::new("Display"),
        vec![EdnTag::new("display")],
        vec![Arc::new(CalcitTypeAnnotation::DynFn)],
      )
      .with_definition_ref("app.display", "Display"),
    );
    let mut composite = CalcitTrait::new(
      EdnTag::new("Widget"),
      vec![EdnTag::new("render")],
      vec![Arc::new(CalcitTypeAnnotation::DynFn)],
    )
    .with_definition_ref("app.widget", "Widget");
    composite.requires = Arc::new(vec![required.clone()]);
    let annotation = CalcitTypeAnnotation::Trait(Arc::new(composite));
    assert!(annotation.satisfies_trait_bound(required.as_ref()));

    let invalid = CalcitTrait {
      runtime_id: None,
      definition_ref: Some(Arc::from("app.invalid/Broken")),
      name: EdnTag::new("Broken"),
      methods: Arc::new(vec![EdnTag::new("missing")]),
      defaults: Arc::new(vec![None]),
      method_types: Arc::new(vec![]),
      member_kinds: Arc::new(vec![crate::calcit::CalcitTraitMemberKind::Method]),
      requires: Arc::new(vec![]),
    };
    let invalid_annotation = CalcitTypeAnnotation::Trait(Arc::new(invalid.clone()));
    assert!(!invalid_annotation.satisfies_trait_bound(&invalid));
  }

  #[test]
  fn nominal_trait_matching_does_not_reverse_unqualified_placeholders() {
    let bare_placeholder = CalcitTrait::new_reference("DomElement");
    let runtime_trait = CalcitTrait::new_runtime(EdnTag::new("DomElement"), vec![], vec![]);

    assert!(
      CalcitTypeAnnotation::trait_references_match(&runtime_trait, &bare_placeholder),
      "an explicitly bare expected trait remains a legacy name-only contract"
    );
    assert!(
      !CalcitTypeAnnotation::trait_references_match(&bare_placeholder, &runtime_trait),
      "a bare actual placeholder must not satisfy a runtime-identified trait by reverse matching"
    );

    let qualified_placeholder = CalcitTrait::new_reference("respo.dom/DomElement");
    let qualified_runtime =
      CalcitTrait::new_runtime(EdnTag::new("DomElement"), vec![], vec![]).with_definition_ref("respo.dom", "DomElement");
    assert!(!qualified_placeholder.matches_reference(&qualified_runtime));
    assert!(
      CalcitTypeAnnotation::trait_references_match(&qualified_placeholder, &qualified_runtime),
      "qualified definition identity keeps the intentional reverse-match path"
    );
  }

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
  fn strict_parser_keeps_recursive_struct_name_as_type_ref() {
    let self_symbol = Calcit::Symbol {
      sym: Arc::from("Node"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests"),
        at_def: Arc::from("Node"),
      }),
      location: None,
    };
    let optional_node = Calcit::from(vec![Calcit::Proc(CalcitProc::NativeEnum), symbol("Optional"), self_symbol]);

    let parsed = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(&optional_node, &[]);
    assert!(
      matches!(
        parsed.as_ref(),
        CalcitTypeAnnotation::Optional(inner)
          if matches!(inner.as_ref(), CalcitTypeAnnotation::TypeRef(name, args) if name.as_ref() == "Node" && args.is_empty())
      ),
      "recursive field annotation must remain a finite TypeRef, got {parsed:?}"
    );
  }

  #[test]
  fn unscoped_assertion_parser_keeps_qualified_name_as_type_ref() {
    let parsed = CalcitTypeAnnotation::parse_type_annotation_form(&symbol("'app.schema/Store"));
    assert!(
      matches!(
        parsed.as_ref(),
        CalcitTypeAnnotation::TypeRef(name, args) if name.as_ref() == "app.schema/Store" && args.is_empty()
      ),
      "expected qualified type reference, got {parsed:?}"
    );
  }

  #[test]
  fn parses_map_wrapped_data_definition_forms() {
    let struct_code = CalcitList::from(&[
      symbol("defstruct"),
      symbol("Store"),
      Calcit::from(vec![symbol("{}"), Calcit::from(vec![Calcit::tag("text"), symbol("'String")])]),
    ] as &[Calcit]);
    let struct_def = parse_defstruct_code(&struct_code).expect("map-wrapped struct definition");
    assert_eq!(struct_def.definition_ref.as_deref(), Some("tests/Store"));
    assert_eq!(struct_def.fields.as_ref(), &[EdnTag::new("text")]);
    assert_eq!(struct_def.field_types.len(), 1);

    let enum_code = CalcitList::from(&[
      symbol("defenum"),
      symbol("Choice"),
      Calcit::from(vec![
        symbol("{}"),
        Calcit::from(vec![Calcit::tag("some"), symbol("'String")]),
        Calcit::from(vec![Calcit::tag("none")]),
      ]),
    ] as &[Calcit]);
    let enum_def = parse_defenum_code(&enum_code).expect("map-wrapped enum definition");
    assert_eq!(enum_def.definition_ref().map(AsRef::as_ref), Some("tests/Choice"));
    assert!(enum_def.find_variant_by_name("some").is_some());
    assert!(enum_def.find_variant_by_name("none").is_some());
  }

  #[test]
  fn nominal_generic_declaration_order_matches_static_and_runtime_paths() {
    let quoted = |name: &str| Calcit::from(vec![symbol("quote"), symbol(name)]);
    // Include a non-adjacent duplicate to preserve the previous deduplication behavior.
    let generics = Calcit::from(vec![Calcit::Proc(CalcitProc::List), quoted("T"), quoted("E"), quoted("T")]);
    let fields = [
      Calcit::from(vec![Calcit::tag("value"), quoted("T")]),
      Calcit::from(vec![Calcit::tag("error"), quoted("E")]),
    ];
    let expected = vec![Arc::<str>::from("T"), Arc::<str>::from("E")];
    let args = vec![symbol("Ordered"), generics, fields[0].clone(), fields[1].clone()];
    let struct_code = CalcitList::from(&[vec![symbol("defstruct")], args.clone()].concat()[..]);
    let enum_code = CalcitList::from(&[vec![symbol("defenum")], args.clone()].concat()[..]);
    let static_struct = parse_defstruct_code(&struct_code).expect("static struct");
    let static_enum = parse_defenum_code(&enum_code).expect("static enum");
    let Calcit::StructDef(runtime_struct) = crate::builtins::structs::new_struct(&args).expect("runtime struct") else {
      panic!("expected StructDef");
    };
    let Calcit::EnumDef(runtime_enum) = crate::builtins::structs::new_enum(&args).expect("runtime enum") else {
      panic!("expected EnumDef");
    };
    assert_eq!(static_struct.generics.as_ref(), &expected);
    assert_eq!(runtime_struct.generics.as_ref(), &expected);
    assert_eq!(static_enum.generics(), expected.as_slice());
    assert_eq!(runtime_enum.generics(), expected.as_slice());
    // Field/variant sorting is independent of generic parameter order.
    assert_eq!(static_struct.fields.as_ref(), &[EdnTag::new("error"), EdnTag::new("value")]);
  }

  fn generic_result_enum() -> Arc<CalcitEnumDef> {
    Arc::new(
      CalcitEnumDef::from_struct(CalcitStructValue {
        struct_ref: Arc::new(CalcitStructDef {
          definition_ref: None,
          name: EdnTag::new("Result"),
          fields: Arc::new(vec![EdnTag::new("err"), EdnTag::new("ok")]),
          field_types: Arc::new(vec![crate::calcit::DYNAMIC_TYPE.clone(); 2]),
          generics: Arc::new(vec![Arc::from("T"), Arc::from("E")]),
          where_bounds: Arc::new(vec![]),
          impls: vec![],
        }),
        values: Arc::new(vec![
          Calcit::List(Arc::new(CalcitList::Vector(vec![symbol("E")]))),
          Calcit::List(Arc::new(CalcitList::Vector(vec![symbol("T")]))),
        ]),
      })
      .expect("valid generic enum"),
    )
  }

  #[test]
  fn trailing_option_sugar_requires_a_core_option_reference() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let core_short = CalcitTypeAnnotation::TypeRef(Arc::from("Option"), Arc::new(vec![number.clone()]));
    let core_qualified = CalcitTypeAnnotation::TypeRef(Arc::from("calcit.core/Option"), Arc::new(vec![number.clone()]));
    let user_qualified = CalcitTypeAnnotation::TypeRef(Arc::from("app.main/Option"), Arc::new(vec![number.clone()]));
    assert!(core_short.is_option_type());
    assert!(core_qualified.is_option_type());
    assert!(!user_qualified.is_option_type());

    let user_option = CalcitEnumDef::from_struct(CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef {
        definition_ref: None,
        name: EdnTag::new("Option"),
        fields: Arc::new(vec![EdnTag::new("some"), EdnTag::new("none")]),
        field_types: Arc::new(vec![crate::calcit::DYNAMIC_TYPE.clone(); 2]),
        generics: Arc::new(vec![Arc::from("T")]),
        where_bounds: Arc::new(vec![]),
        impls: vec![],
      }),
      values: Arc::new(vec![Calcit::from(vec![symbol("T")]), Calcit::Nil]),
    })
    .expect("valid user Option enum");
    let resolved_user_option = CalcitTypeAnnotation::Enum(Arc::new(user_option), Arc::new(vec![number]));
    assert!(!resolved_user_option.is_option_type());
  }

  #[test]
  fn struct_and_enum_definitions_do_not_match_instance_types() {
    let person = Arc::new(CalcitStructDef::from_fields(EdnTag::new("Person"), vec![EdnTag::new("name")]));
    let result = generic_result_enum();

    let person_def = CalcitTypeAnnotation::StructDef(person.clone());
    let person_value = CalcitTypeAnnotation::StructValue(person.clone());
    let result_def = CalcitTypeAnnotation::EnumDef(result.clone());
    let result_value = CalcitTypeAnnotation::EnumValue(result.clone());

    assert!(!person_def.is_compatible_with(&CalcitTypeAnnotation::from_tag_name("Struct")));
    assert!(person_def.is_compatible_with(&CalcitTypeAnnotation::from_tag_name("StructDef")));
    assert!(person_value.is_compatible_with(&CalcitTypeAnnotation::from_tag_name("Struct")));
    assert!(!person_value.is_compatible_with(&CalcitTypeAnnotation::from_tag_name("StructDef")));

    assert!(!result_def.is_compatible_with(&CalcitTypeAnnotation::from_tag_name("Enum")));
    assert!(result_def.is_compatible_with(&CalcitTypeAnnotation::from_tag_name("EnumDef")));
    assert!(result_value.is_compatible_with(&CalcitTypeAnnotation::from_tag_name("Enum")));
    assert!(!result_value.is_compatible_with(&CalcitTypeAnnotation::from_tag_name("EnumDef")));

    assert_eq!(person_def.to_brief_string(), "struct-def Person");
    assert_eq!(result_def.to_brief_string(), "enum-def Result");

    let person_definition = Calcit::StructDef(person.as_ref().clone());
    let person_instance = Calcit::Struct(CalcitStructValue {
      struct_ref: person.clone(),
      values: Arc::new(vec![Calcit::Str(Arc::from("Ada"))]),
    });
    assert!(value_matches_type_annotation(&person_definition, &person_def));
    assert!(!value_matches_type_annotation(&person_definition, &person_value));
    assert!(value_matches_type_annotation(&person_instance, &person_value));
    assert!(!value_matches_type_annotation(&person_instance, &person_def));
  }

  #[test]
  fn entry_type_slots_are_global_defaults_but_scoped_overrides_still_win() {
    clear_type_slots();
    let bindings = HashMap::from([("dispatch-op".to_owned(), "app.schema/Op".to_owned())]);
    configure_entry_type_slots(&bindings).expect("configure entry type slots");

    assert_eq!(
      resolve_type_slot("dispatch-op"),
      Some(Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from("app.schema/Op"),
        Arc::new(vec![])
      )))
    );

    push_type_slot_override(Arc::from("dispatch-op"), Arc::new(CalcitTypeAnnotation::String));
    assert_eq!(resolve_type_slot("dispatch-op"), Some(Arc::new(CalcitTypeAnnotation::String)));
    pop_type_slot_override("dispatch-op");
    assert!(matches!(
      resolve_type_slot("dispatch-op").as_deref(),
      Some(CalcitTypeAnnotation::TypeRef(path, _)) if path.as_ref() == "app.schema/Op"
    ));
    clear_type_slots();
  }

  #[test]
  fn entry_type_slots_require_full_type_paths() {
    clear_type_slots();
    let bindings = HashMap::from([("dispatch-op".to_owned(), "Op".to_owned())]);
    let error = configure_entry_type_slots(&bindings).expect_err("short type path should fail");
    assert!(error.contains("namespace/definition"), "unexpected error: {error}");
    assert!(resolve_type_slot("dispatch-op").is_none());
  }

  #[test]
  fn generic_map_type_ref_accepts_structs_structurally() {
    // Structs are field-name -> value, structurally map-like, so a proc typed
    // as accepting a generic "map" (e.g. `to-pairs`/`keys`) should not warn
    // when called with a struct. See RFC 07-19-type-introspection-consistency.
    let person_struct = CalcitStructDef::from_fields(EdnTag::new("Person"), vec![EdnTag::new("name")]);
    let struct_type = CalcitTypeAnnotation::StructValue(Arc::new(person_struct));
    let map_type = CalcitTypeAnnotation::TypeRef(Arc::from("map"), Arc::new(vec![]));

    let mut bindings = TypeBindings::new();
    assert!(struct_type.compatible_with_bindings(&map_type, &mut bindings));
    assert!(map_type.compatible_with_bindings(&struct_type, &mut bindings));

    // unrelated type-ref names still must not match a struct structurally
    let person_type = CalcitTypeAnnotation::TypeRef(Arc::from("SomeOtherName"), Arc::new(vec![]));
    let mut bindings2 = TypeBindings::new();
    assert!(!struct_type.compatible_with_bindings(&person_type, &mut bindings2));
  }

  #[test]
  fn repeated_identical_type_vars_do_not_create_recursive_self_bindings() {
    let type_var = CalcitTypeAnnotation::TypeVar(Arc::from("T"));
    let mut bindings = TypeBindings::new();

    assert!(type_var.compatible_with_bindings(&type_var, &mut bindings));
    assert!(type_var.compatible_with_bindings(&type_var, &mut bindings));
    assert!(bindings.is_empty(), "an identical type variable needs no self-binding");
  }

  #[test]
  fn nested_type_vars_do_not_create_recursive_occurs_bindings() {
    let type_var = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let optional_type_var = CalcitTypeAnnotation::Optional(type_var);
    let expected_type_var = CalcitTypeAnnotation::TypeVar(Arc::from("T"));
    let mut bindings = TypeBindings::new();

    assert!(optional_type_var.compatible_with_bindings(&expected_type_var, &mut bindings));
    assert!(bindings.is_empty(), "T must not be bound to Optional<T>");

    assert!(CalcitTypeAnnotation::Tag.compatible_with_bindings(&expected_type_var, &mut bindings));
    assert_eq!(bindings.get("T").map(AsRef::as_ref), Some(&CalcitTypeAnnotation::Tag));

    let reverse_type_var = CalcitTypeAnnotation::TypeVar(Arc::from("T"));
    let reverse_optional = CalcitTypeAnnotation::Optional(Arc::new(reverse_type_var.clone()));
    let mut reverse_bindings = TypeBindings::new();
    assert!(reverse_type_var.compatible_with_bindings(&reverse_optional, &mut reverse_bindings));
    assert!(
      reverse_bindings.is_empty(),
      "T must not be bound to Optional<T> in the reverse direction"
    );
    assert!(reverse_type_var.compatible_with_bindings(&CalcitTypeAnnotation::String, &mut reverse_bindings));
    assert_eq!(reverse_bindings.get("T").map(AsRef::as_ref), Some(&CalcitTypeAnnotation::String));
  }

  fn nested_list_type(depth: usize, leaf: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
    (0..depth).fold(leaf, |inner, _| Arc::new(CalcitTypeAnnotation::List(inner)))
  }

  #[test]
  fn deep_type_analysis_uses_bounded_iterative_paths() {
    let depth = 2_048;
    let actual = nested_list_type(depth, Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T"))));
    let expected = nested_list_type(depth, Arc::new(CalcitTypeAnnotation::String));
    let mut bindings = TypeBindings::new();
    bindings.insert(Arc::from("T"), Arc::new(CalcitTypeAnnotation::String));

    let started = std::time::Instant::now();
    assert!(actual.contains_type_var());
    let substituted = actual.substitute_type_vars(&bindings);
    assert!(!substituted.contains_type_var());
    assert!(substituted.is_compatible_with(&expected));
    let diagnostic = substituted.describe();
    let elapsed = started.elapsed();

    assert!(diagnostic.contains('…'), "deep diagnostics must preserve a truncation marker");
    assert!(
      diagnostic.len() < 512,
      "deep diagnostics must stay bounded, got {} bytes",
      diagnostic.len()
    );
    eprintln!(
      "type-analysis depth={depth} visits={} elapsed_us={}",
      depth + 1,
      elapsed.as_micros()
    );
  }

  #[test]
  fn deep_map_option_and_nominal_relations_remain_precise() {
    for depth in [32usize, 256, 2_048] {
      let map_actual = (0..depth).fold(Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T"))), |value, _| {
        Arc::new(CalcitTypeAnnotation::Map(Arc::new(CalcitTypeAnnotation::String), value))
      });
      let map_expected = (0..depth).fold(Arc::new(CalcitTypeAnnotation::Number), |value, _| {
        Arc::new(CalcitTypeAnnotation::Map(Arc::new(CalcitTypeAnnotation::String), value))
      });
      let option_actual = (0..depth).fold(Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T"))), |value, _| {
        Arc::new(CalcitTypeAnnotation::TypeRef(
          Arc::from("calcit.core/Option"),
          Arc::new(vec![value]),
        ))
      });
      let option_expected = (0..depth).fold(Arc::new(CalcitTypeAnnotation::Number), |value, _| {
        Arc::new(CalcitTypeAnnotation::TypeRef(
          Arc::from("calcit.core/Option"),
          Arc::new(vec![value]),
        ))
      });
      let nominal_actual = (0..depth).fold(Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T"))), |value, _| {
        Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("tests/Node"), Arc::new(vec![value])))
      });
      let nominal_expected = (0..depth).fold(Arc::new(CalcitTypeAnnotation::Number), |value, _| {
        Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("tests/Node"), Arc::new(vec![value])))
      });

      for (actual, expected) in [
        (&map_actual, &map_expected),
        (&option_actual, &option_expected),
        (&nominal_actual, &nominal_expected),
      ] {
        let mut bindings = TypeBindings::new();
        assert!(actual.compatible_with_bindings(expected, &mut bindings));
        assert_eq!(bindings.get("T").map(Arc::as_ref), Some(&CalcitTypeAnnotation::Number));
      }
    }
  }

  #[test]
  fn substituted_shared_subtypes_keep_arc_identity() {
    let shared = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("tests/Shared"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))]),
    ));
    let root = CalcitTypeAnnotation::TypeRef(Arc::from("tests/Pair"), Arc::new(vec![shared.clone(), shared]));
    let bindings = TypeBindings::from([(Arc::from("T"), Arc::new(CalcitTypeAnnotation::String))]);

    let substituted = root.substitute_type_vars(&bindings);
    let CalcitTypeAnnotation::TypeRef(_, args) = substituted.as_ref() else {
      panic!("expected substituted named pair");
    };
    assert!(Arc::ptr_eq(&args[0], &args[1]));
    assert!(!substituted.contains_type_var());
  }

  #[test]
  fn relation_budget_rejects_oversized_shapes_without_dynamic_fallback() {
    let oversized = TYPE_RELATION_NODE_LIMIT + 1;
    let actual = CalcitTypeAnnotation::TypeRef(
      Arc::from("app/Wide"),
      Arc::new((0..oversized).map(|_| Arc::new(CalcitTypeAnnotation::String)).collect()),
    );
    let expected = CalcitTypeAnnotation::TypeRef(
      Arc::from("app/Wide"),
      Arc::new((0..oversized).map(|_| Arc::new(CalcitTypeAnnotation::String)).collect()),
    );

    assert!(!actual.is_compatible_with(&expected));
  }

  #[test]
  fn recursive_type_slot_relation_is_rejected_and_scope_is_cleaned_up() {
    let left: Arc<str> = Arc::from("tests/Left");
    let right: Arc<str> = Arc::from("tests/Right");
    push_type_slot_override(left.clone(), Arc::new(CalcitTypeAnnotation::TypeSlot(right.clone())));
    push_type_slot_override(right.clone(), Arc::new(CalcitTypeAnnotation::TypeSlot(left.clone())));

    let recursive = CalcitTypeAnnotation::TypeSlot(left.clone());
    assert!(!recursive.is_compatible_with(&CalcitTypeAnnotation::Number));
    assert_eq!(
      recursive.prove_with_bindings(&CalcitTypeAnnotation::Number, &mut TypeBindings::new()),
      TypeProof::NeedsBoundary(TypeBoundaryReason::RecursiveTypeSlot)
    );

    pop_type_slot_override(&right);
    pop_type_slot_override(&left);
    assert!(recursive.is_compatible_with(&CalcitTypeAnnotation::Number));
  }

  #[test]
  fn generic_occurs_check_follows_existing_alias_bindings() {
    let generic_t = CalcitTypeAnnotation::TypeVar(Arc::from("T"));
    let generic_u = CalcitTypeAnnotation::TypeVar(Arc::from("U"));
    let optional_t = CalcitTypeAnnotation::Optional(Arc::new(generic_t.clone()));
    let mut bindings = TypeBindings::new();

    assert!(generic_u.compatible_with_bindings(&generic_t, &mut bindings));
    assert_eq!(bindings.get("T").map(AsRef::as_ref), Some(&generic_u));

    assert!(optional_t.compatible_with_bindings(&generic_t, &mut bindings));
    assert!(
      !bindings.contains_key("U"),
      "U must not be bound to Optional<T> through the T -> U alias"
    );

    assert!(CalcitTypeAnnotation::Tag.compatible_with_bindings(&generic_t, &mut bindings));
    assert_eq!(bindings.get("U").map(AsRef::as_ref), Some(&CalcitTypeAnnotation::Tag));
  }

  #[test]
  fn sibling_callbacks_share_generic_return_bindings() {
    let generic_t = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let generic_u = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("U")));
    let expected_none = CalcitTypeAnnotation::from_function_parts(vec![], generic_u.clone());
    let expected_some = CalcitTypeAnnotation::from_function_parts(vec![generic_t], generic_u);
    let actual_none = CalcitTypeAnnotation::from_function_parts(vec![], Arc::new(CalcitTypeAnnotation::String));
    let actual_some =
      CalcitTypeAnnotation::from_function_parts(vec![Arc::new(CalcitTypeAnnotation::Number)], Arc::new(CalcitTypeAnnotation::Number));
    let mut bindings = TypeBindings::new();

    assert!(actual_none.compatible_with_bindings(&expected_none, &mut bindings));
    assert!(
      !actual_some.compatible_with_bindings(&expected_some, &mut bindings),
      "a sibling callback must return the U bound by the first callback"
    );
  }

  #[test]
  fn failed_callback_signature_does_not_leak_partial_generic_bindings() {
    let generic = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let expected = CalcitTypeAnnotation::from_function_parts(vec![generic.clone(), generic], Arc::new(CalcitTypeAnnotation::Unit));
    let actual = CalcitTypeAnnotation::from_function_parts(
      vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)],
      Arc::new(CalcitTypeAnnotation::Unit),
    );
    let mut bindings = TypeBindings::new();

    assert!(!actual.compatible_with_bindings(&expected, &mut bindings));
    assert!(bindings.is_empty(), "a failed callback match must not constrain later arguments");
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
  fn async_fn_schema_roundtrips_and_changes_callback_contract() {
    let schema_form = Calcit::from(vec![
      symbol("{}"),
      Calcit::from(vec![Calcit::Tag(EdnTag::from("args")), Calcit::from(vec![symbol("[]")])]),
      Calcit::from(vec![Calcit::Tag(EdnTag::from("return")), symbol("String")]),
      Calcit::from(vec![Calcit::Tag(EdnTag::from("async")), Calcit::Bool(true)]),
    ]);
    let hint = Calcit::from(vec![Calcit::Syntax(CalcitSyntax::HintFn, Arc::from("tests")), schema_form]);
    let annotation = CalcitTypeAnnotation::extract_fn_annotation_from_hint_form(&hint).expect("async fn annotation");
    let CalcitTypeAnnotation::Fn(async_signature) = annotation.as_ref() else {
      panic!("expected fn annotation")
    };
    assert!(async_signature.is_async_invocation());

    let serialized = async_signature.to_schema_edn();
    let parsed = CalcitTypeAnnotation::parse_fn_schema_from_edn(&serialized).expect("roundtrip async schema");
    assert!(parsed.is_async_invocation());

    let sync_signature = CalcitFnTypeAnnotation {
      features: Arc::new(HashSet::new()),
      ..async_signature.as_ref().clone()
    };
    assert!(!async_signature.matches_signature(&sync_signature));
    assert!(!sync_signature.matches_signature(async_signature));
    assert_ne!(
      CalcitTypeAnnotation::Fn(async_signature.clone()).cmp(&CalcitTypeAnnotation::Fn(Arc::new(sync_signature))),
      Ordering::Equal
    );
  }

  #[test]
  fn async_only_schema_is_valid_but_target_hint_does_not_mark_definition() {
    let async_schema = Calcit::from(vec![
      symbol("{}"),
      Calcit::from(vec![Calcit::Tag(EdnTag::from("async")), Calcit::Bool(true)]),
    ]);
    let definition_hint = Calcit::from(vec![Calcit::Syntax(CalcitSyntax::HintFn, Arc::from("tests")), async_schema.clone()]);

    let annotation = CalcitTypeAnnotation::extract_fn_annotation_from_hint_form(&definition_hint).expect("async-only fn schema");
    assert!(matches!(annotation.as_ref(), CalcitTypeAnnotation::Fn(signature) if signature.is_async_invocation()));
    assert!(CalcitTypeAnnotation::hint_form_marks_async(&definition_hint));

    let target_hint = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::HintFn, Arc::from("tests")),
      symbol("target"),
      async_schema,
    ]);
    assert!(!CalcitTypeAnnotation::hint_form_marks_async(&target_hint));
  }

  #[test]
  fn reserved_async_feature_cannot_override_explicit_false() {
    let reserved = EdnTag::from(ASYNC_INVOCATION_FEATURE);
    let source_schema = Calcit::from(vec![
      symbol("{}"),
      Calcit::from(vec![
        Calcit::Tag(EdnTag::from("features")),
        Calcit::Set([Calcit::Tag(reserved.clone())].into_iter().collect()),
      ]),
      Calcit::from(vec![Calcit::Tag(EdnTag::from("async")), Calcit::Bool(false)]),
    ]);
    let source_hint = Calcit::from(vec![Calcit::Syntax(CalcitSyntax::HintFn, Arc::from("tests")), source_schema]);
    let source = CalcitTypeAnnotation::extract_fn_annotation_from_hint_form(&source_hint).expect("source fn schema");
    let CalcitTypeAnnotation::Fn(source) = source.as_ref() else {
      panic!("expected source fn annotation")
    };
    assert!(!source.is_async_invocation());
    assert!(!source.features.contains(&reserved));

    let mut features = EdnSetView::default();
    features.insert(Edn::Tag(reserved.clone()));
    let edn_schema = Edn::Map(EdnMapView::from(HashMap::from([
      (Edn::tag("features"), Edn::Set(features)),
      (Edn::tag("async"), Edn::Bool(false)),
    ])));
    let edn = CalcitTypeAnnotation::parse_fn_schema_from_edn(&edn_schema).expect("EDN fn schema");
    assert!(!edn.is_async_invocation());
    assert!(!edn.features.contains(&reserved));
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
  fn unscoped_parser_keeps_applied_quoted_name_as_type_ref() {
    let applied = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("::"),
      Calcit::List(Arc::new(CalcitList::from(&[
        Calcit::Syntax(CalcitSyntax::Quote, Arc::from(CORE_NS)),
        symbol("Box"),
      ]))),
      Calcit::Tag(EdnTag::from("number")),
    ])));

    let parsed = CalcitTypeAnnotation::parse_type_annotation_form(&applied);
    assert!(matches!(parsed.as_ref(), CalcitTypeAnnotation::TypeRef(name, args)
        if name.as_ref() == "Box"
          && matches!(args.as_slice(), [arg] if matches!(arg.as_ref(), CalcitTypeAnnotation::Number))));
  }

  #[test]
  fn zero_argument_nil_applications_keep_nil_type() {
    let list_form = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("::"),
      Calcit::List(Arc::new(CalcitList::from(&[
        Calcit::Syntax(CalcitSyntax::Quote, Arc::from(CORE_NS)),
        symbol("Nil"),
      ]))),
    ])));
    let enum_form = CalcitTypeAnnotation::edn_type_to_calcit(&Edn::enum_value("Nil", vec![]));

    assert!(matches!(
      CalcitTypeAnnotation::parse_type_annotation_form(&list_form).as_ref(),
      CalcitTypeAnnotation::Nil
    ));
    assert!(matches!(
      CalcitTypeAnnotation::parse_type_annotation_form(&enum_form).as_ref(),
      CalcitTypeAnnotation::Nil
    ));
  }

  #[test]
  fn unscoped_parser_keeps_applied_qualified_name_as_type_ref() {
    let applied = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("::"),
      symbol("'app.schema/Box"),
      symbol("'String"),
    ])));

    let parsed = CalcitTypeAnnotation::parse_type_annotation_form(&applied);
    assert!(matches!(parsed.as_ref(), CalcitTypeAnnotation::TypeRef(name, args)
        if name.as_ref() == "app.schema/Box"
          && matches!(args.as_slice(), [arg] if matches!(arg.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "String"))));
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
      features: Arc::new(HashSet::new()),
    }));

    let edn = annotation.to_type_edn();
    let Edn::Enum(view) = &edn else {
      panic!("fn annotation should serialize as tuple, got {edn:?}");
    };
    assert_eq!(view.variant.as_ref(), "Fn");
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
      features: Arc::new(HashSet::new()),
    }));

    let edn = annotation.to_type_edn();
    let Edn::Enum(view) = &edn else {
      panic!("fn annotation should serialize as tuple, got {edn:?}");
    };
    let Some(Edn::Map(map)) = view.extra.first() else {
      panic!("fn payload should be schema map: {edn:?}");
    };
    assert!(matches!(map.tag_get("kind"), Some(Edn::Tag(tag)) if tag.ref_str() == "macro"));
    assert!(matches!(map.tag_get("return"), Some(Edn::Symbol(name)) if name.as_ref() == "Bool"));
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
    let schema = Edn::enum_value(
      "fn",
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
    let schema = Edn::enum_value(
      "fn",
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
  fn extracts_symbol_trait_placeholder_from_strict_where_hint() {
    let generics = Calcit::List(Arc::new(CalcitList::from(&[symbol("[]"), symbol("T")])));
    let where_map = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("{}"),
      Calcit::List(Arc::new(CalcitList::from(&[symbol("T"), symbol("Show")]))),
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

    let bounds = CalcitTypeAnnotation::extract_where_bounds_from_hint_form(&hint_form).expect("symbol trait bound should parse");
    assert_eq!(bounds.len(), 1);
    assert_eq!(bounds[0].name.as_ref(), "T");
    assert_eq!(bounds[0].traits.len(), 1);
    assert_eq!(bounds[0].traits[0].name.ref_str(), "Show");
    assert!(
      bounds[0].traits[0].methods.is_empty(),
      "source resolution happens during preprocessing"
    );
  }

  #[test]
  fn qualified_trait_bounds_keep_their_nominal_source_reference() {
    let source_form = symbol("respo.dom/DomElement");
    let trait_def = Arc::new(CalcitTrait::new(EdnTag::new("DomElement"), vec![], vec![]));

    let qualified = CalcitTypeAnnotation::trait_bound_with_source_ref(&source_form, &trait_def);

    assert_eq!(qualified.definition_ref.as_deref(), Some("respo.dom/DomElement"));
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
      features: Arc::new(HashSet::new()),
    };

    let edn = schema.to_wrapped_schema_edn();
    let Edn::Enum(view) = edn else {
      panic!("wrapped schema should serialize as tuple");
    };
    let Some(Edn::Map(map)) = view.extra.first() else {
      panic!("wrapped schema payload should be a map");
    };
    let Some(Edn::Map(where_map)) = map.tag_get("where") else {
      panic!("wrapped schema should contain where map");
    };
    assert!(matches!(where_map.0.get(&Edn::Symbol(Arc::from("T"))), Some(Edn::Symbol(name)) if name.as_ref() == "Show"));
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
      features: Arc::new(HashSet::new()),
    };

    let edn = schema.to_wrapped_schema_edn();
    let Edn::Enum(view) = edn else {
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
    assert!(matches!(traits.0.first(), Some(Edn::Symbol(name)) if name.as_ref() == "Show"));
    assert!(matches!(traits.0.get(1), Some(Edn::Symbol(name)) if name.as_ref() == "Eq"));
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
    let schema = Edn::enum_value(
      "macro",
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
      features: Arc::new(HashSet::new()),
    };

    let edn = schema.to_wrapped_schema_edn();
    let Edn::Enum(view) = edn else {
      panic!("wrapped schema should serialize as tuple");
    };
    let Some(Edn::Map(map)) = view.extra.first() else {
      panic!("wrapped schema payload should be a map");
    };
    assert!(map.tag_get("kind").is_none(), "default fn kind should be omitted");
    assert!(matches!(map.tag_get("rest"), Some(Edn::Symbol(name)) if name.as_ref() == "Tag"));
  }

  #[test]
  fn any_is_parsed_and_written_as_a_dynamic_alias() {
    let any = CalcitTypeAnnotation::from_tag_name("any");
    assert!(matches!(
      CalcitTypeAnnotation::from_tag_name("dynamic"),
      CalcitTypeAnnotation::Dynamic
    ));
    assert!(matches!(any, CalcitTypeAnnotation::Dynamic));
    assert!(matches!(
      CalcitTypeAnnotation::from_calcit(&Calcit::Tag(EdnTag::from("any"))),
      CalcitTypeAnnotation::Dynamic
    ));
    assert!(CalcitTypeAnnotation::Number.is_compatible_with(&any));
    assert!(any.is_compatible_with(&CalcitTypeAnnotation::Number));
    assert_eq!(any.to_type_edn(), Edn::Symbol(Arc::from("Dynamic")));

    let list_of_numbers = CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Number));
    let list_of_any = CalcitTypeAnnotation::List(Arc::new(any));
    assert!(list_of_numbers.is_compatible_with(&list_of_any));
    assert!(list_of_any.is_compatible_with(&list_of_numbers));
  }

  #[test]
  fn canonical_symbol_schema_types_parse_like_legacy_tags() {
    let legacy = CalcitTypeAnnotation::parse_type_annotation_from_edn(&Edn::enum_value("list", vec![Edn::tag("string")]));
    let canonical =
      CalcitTypeAnnotation::parse_type_annotation_from_edn(&Edn::enum_value("List", vec![Edn::Symbol(Arc::from("String"))]));
    assert_eq!(canonical, legacy);
    assert_eq!(
      canonical.to_type_edn(),
      Edn::enum_value("List", vec![Edn::Symbol(Arc::from("String"))])
    );
    assert!(matches!(
      CalcitTypeAnnotation::parse_type_annotation_from_edn(&Edn::Symbol(Arc::from("Dynamic"))).as_ref(),
      CalcitTypeAnnotation::Dynamic
    ));
  }

  #[test]
  fn explicit_dynamic_container_arguments_round_trip_without_becoming_bare() {
    for name in ["List", "Set", "Ref"] {
      let explicit =
        CalcitTypeAnnotation::parse_type_annotation_from_edn(&Edn::enum_value(name, vec![Edn::Symbol(Arc::from("Dynamic"))]));
      assert_eq!(
        explicit.to_type_edn(),
        Edn::enum_value(name, vec![Edn::Symbol(Arc::from("Dynamic"))]),
        "explicit {name}<Dynamic> must remain auditable"
      );

      let bare = CalcitTypeAnnotation::parse_type_annotation_from_edn(&Edn::Symbol(Arc::from(name)));
      assert_eq!(
        bare.to_type_edn(),
        Edn::Symbol(Arc::from(name)),
        "bare {name} must remain distinguishable"
      );
    }

    let explicit_map = CalcitTypeAnnotation::parse_type_annotation_from_edn(&Edn::enum_value(
      "Map",
      vec![Edn::Symbol(Arc::from("Dynamic")), Edn::Symbol(Arc::from("Dynamic"))],
    ));
    assert_eq!(
      explicit_map.to_type_edn(),
      Edn::enum_value("Map", vec![Edn::Symbol(Arc::from("Dynamic")), Edn::Symbol(Arc::from("Dynamic"))])
    );
    let bare_map = CalcitTypeAnnotation::parse_type_annotation_from_edn(&Edn::Symbol(Arc::from("Map")));
    assert_eq!(bare_map.to_type_edn(), Edn::Symbol(Arc::from("Map")));

    let nested = CalcitTypeAnnotation::parse_type_annotation_from_edn(&Edn::enum_value(
      "List",
      vec![Edn::enum_value("List", vec![Edn::Symbol(Arc::from("Dynamic"))])],
    ));
    assert_eq!(
      nested.to_type_edn(),
      Edn::enum_value("List", vec![Edn::enum_value("List", vec![Edn::Symbol(Arc::from("Dynamic"))])])
    );
  }

  #[test]
  fn rejects_type_args_on_non_generic_struct_annotation() {
    let pair = CalcitStructDef {
      definition_ref: None,
      name: EdnTag::new("Pair"),
      fields: Arc::new(vec![]),
      field_types: Arc::new(vec![]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
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
    let pair = CalcitStructDef {
      definition_ref: None,
      name: EdnTag::new("Pair"),
      fields: Arc::new(vec![]),
      field_types: Arc::new(vec![]),
      generics: Arc::new(vec![Arc::from("A"), Arc::from("B")]),
      where_bounds: Arc::new(vec![]),
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
    let pair = Arc::new(CalcitStructDef {
      definition_ref: None,
      name: EdnTag::new("Pair"),
      fields: Arc::new(vec![]),
      field_types: Arc::new(vec![]),
      generics: Arc::new(vec![Arc::from("A"), Arc::from("B")]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let actual = CalcitTypeAnnotation::Struct(
      pair.clone(),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)]),
    );
    let expected = CalcitTypeAnnotation::TypeRef(Arc::from("Pair"), Arc::new(vec![]));
    let mut bindings = TypeBindings::new();

    assert!(actual.compatible_with_bindings(&expected, &mut bindings));
    assert!(matches!(bindings.get("A"), Some(bound) if matches!(bound.as_ref(), CalcitTypeAnnotation::Number)));
    assert!(matches!(bindings.get("B"), Some(bound) if matches!(bound.as_ref(), CalcitTypeAnnotation::String)));
  }

  #[test]
  fn matching_bare_struct_annotation_binds_generic_args_from_named_struct_ref() {
    let pair = Arc::new(CalcitStructDef {
      definition_ref: None,
      name: EdnTag::new("Pair"),
      fields: Arc::new(vec![]),
      field_types: Arc::new(vec![]),
      generics: Arc::new(vec![Arc::from("A"), Arc::from("B")]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let actual = CalcitTypeAnnotation::TypeRef(
      Arc::from("Pair"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)]),
    );
    let expected = CalcitTypeAnnotation::Struct(pair, Arc::new(vec![]));
    let mut bindings = TypeBindings::new();

    assert!(actual.compatible_with_bindings(&expected, &mut bindings));
    assert!(matches!(bindings.get("A"), Some(bound) if matches!(bound.as_ref(), CalcitTypeAnnotation::Number)));
    assert!(matches!(bindings.get("B"), Some(bound) if matches!(bound.as_ref(), CalcitTypeAnnotation::String)));
  }

  #[test]
  fn matching_named_enum_ref_binds_generic_args_from_enum_annotation() {
    let result = generic_result_enum();
    let actual = CalcitTypeAnnotation::Enum(
      result.clone(),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)]),
    );
    let expected = CalcitTypeAnnotation::TypeRef(Arc::from("Result"), Arc::new(vec![]));
    let mut bindings = TypeBindings::new();

    assert!(actual.compatible_with_bindings(&expected, &mut bindings));
    assert!(matches!(bindings.get("T"), Some(bound) if matches!(bound.as_ref(), CalcitTypeAnnotation::Number)));
    assert!(matches!(bindings.get("E"), Some(bound) if matches!(bound.as_ref(), CalcitTypeAnnotation::String)));
  }

  #[test]
  fn matching_bare_enum_annotation_binds_generic_args_from_named_enum_ref() {
    let result = generic_result_enum();
    let actual = CalcitTypeAnnotation::TypeRef(
      Arc::from("Result"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)]),
    );
    let expected = CalcitTypeAnnotation::Enum(result, Arc::new(vec![]));
    let mut bindings = TypeBindings::new();

    assert!(actual.compatible_with_bindings(&expected, &mut bindings));
    assert!(matches!(bindings.get("T"), Some(bound) if matches!(bound.as_ref(), CalcitTypeAnnotation::Number)));
    assert!(matches!(bindings.get("E"), Some(bound) if matches!(bound.as_ref(), CalcitTypeAnnotation::String)));
  }

  #[test]
  fn proof_distinguishes_legacy_compatibility_boundaries() {
    let mut bindings = TypeBindings::new();
    assert_eq!(
      CalcitTypeAnnotation::Dynamic.prove_with_bindings(&CalcitTypeAnnotation::Number, &mut bindings),
      TypeProof::NeedsBoundary(TypeBoundaryReason::Dynamic)
    );
    assert_eq!(
      CalcitTypeAnnotation::Tag.prove_with_bindings(&CalcitTypeAnnotation::DynFn, &mut bindings),
      TypeProof::NeedsBoundary(TypeBoundaryReason::TagCallable)
    );
    assert_eq!(
      CalcitTypeAnnotation::TypeSlot(Arc::from("unbound")).prove_with_bindings(&CalcitTypeAnnotation::String, &mut bindings),
      TypeProof::NeedsBoundary(TypeBoundaryReason::UnresolvedTypeSlot)
    );
    assert!(bindings.is_empty());
  }

  #[test]
  fn proof_preserves_direction_when_the_expected_type_slot_resolves() {
    clear_type_slots();
    let slot_name: Arc<str> = Arc::from("dispatch");
    let concrete_enum = Arc::new(CalcitTypeAnnotation::Enum(generic_result_enum(), Arc::new(vec![])));
    push_type_slot_override(slot_name.clone(), concrete_enum.clone());

    let mut bindings = TypeBindings::new();
    let anonymous = CalcitTypeAnnotation::AnonymousEnum;
    let slot = CalcitTypeAnnotation::TypeSlot(slot_name.clone());
    assert_eq!(anonymous.prove_with_bindings(&slot, &mut bindings), TypeProof::Mismatch);
    assert_eq!(
      concrete_enum.prove_with_bindings(&CalcitTypeAnnotation::AnonymousEnum, &mut bindings),
      TypeProof::Proven
    );

    pop_type_slot_override(&slot_name);
    clear_type_slots();
  }

  #[test]
  fn proof_is_transactional_and_does_not_synthesize_optional_bindings() {
    let var: Arc<str> = Arc::from("T");
    let expected = CalcitTypeAnnotation::TypeVar(var.clone());
    let mut bindings = TypeBindings::new();

    assert_eq!(
      CalcitTypeAnnotation::Nil.prove_with_bindings(&expected, &mut bindings),
      TypeProof::Proven
    );
    assert!(matches!(bindings.get(&var).map(AsRef::as_ref), Some(CalcitTypeAnnotation::Nil)));

    let before = bindings.clone();
    assert_eq!(
      CalcitTypeAnnotation::Number.prove_with_bindings(&expected, &mut bindings),
      TypeProof::Mismatch
    );
    assert_eq!(bindings, before, "a failed sibling proof must not widen or leak bindings");
  }

  #[test]
  fn proof_rejects_dynamic_inside_ref_and_erased_applied_arguments() {
    let mut bindings = TypeBindings::new();
    let dynamic_ref = CalcitTypeAnnotation::Ref(Arc::new(CalcitTypeAnnotation::Dynamic));
    let number_ref = CalcitTypeAnnotation::Ref(Arc::new(CalcitTypeAnnotation::Number));
    assert_eq!(
      dynamic_ref.prove_with_bindings(&number_ref, &mut bindings),
      TypeProof::NeedsBoundary(TypeBoundaryReason::Dynamic)
    );
    assert_eq!(
      number_ref.prove_with_bindings(&dynamic_ref, &mut bindings),
      TypeProof::NeedsBoundary(TypeBoundaryReason::Dynamic)
    );

    let optional_number_ref =
      CalcitTypeAnnotation::Ref(Arc::new(CalcitTypeAnnotation::Optional(Arc::new(CalcitTypeAnnotation::Number))));
    assert!(!number_ref.prove_with_bindings(&optional_number_ref, &mut bindings).is_proven());

    let bare = CalcitTypeAnnotation::TypeRef(Arc::from("app/Box"), Arc::new(vec![]));
    let applied = CalcitTypeAnnotation::TypeRef(Arc::from("app/Box"), Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]));
    assert!(bare.is_compatible_with(&applied));
    assert_eq!(
      bare.prove_with_bindings(&applied, &mut bindings),
      TypeProof::NeedsBoundary(TypeBoundaryReason::ErasedTypeArguments)
    );
  }

  #[test]
  fn nominal_proof_uses_qualified_definition_and_schema_identity() {
    let user = |namespace: &str, field_type: Arc<CalcitTypeAnnotation>| {
      Arc::new(CalcitStructDef {
        definition_ref: Some(Arc::from(format!("{namespace}/User"))),
        name: EdnTag::new("User"),
        fields: Arc::new(vec![EdnTag::new("id")]),
        field_types: Arc::new(vec![field_type]),
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        impls: vec![],
      })
    };
    let app_user = user("app.models", Arc::new(CalcitTypeAnnotation::Number));
    let admin_user = user("admin.models", Arc::new(CalcitTypeAnnotation::Number));
    let reloaded_user = user("app.models", Arc::new(CalcitTypeAnnotation::String));

    let app_type = CalcitTypeAnnotation::Struct(app_user.clone(), Arc::new(vec![]));
    let admin_type = CalcitTypeAnnotation::Struct(admin_user, Arc::new(vec![]));
    let reloaded_type = CalcitTypeAnnotation::Struct(reloaded_user, Arc::new(vec![]));
    assert!(!app_type.is_compatible_with(&admin_type));
    assert_eq!(
      app_type.prove_with_bindings(&admin_type, &mut TypeBindings::new()),
      TypeProof::Mismatch
    );
    assert_eq!(
      app_type.prove_with_bindings(&reloaded_type, &mut TypeBindings::new()),
      TypeProof::Mismatch,
      "a schema change at the same declaration path must invalidate an old proof"
    );

    let mut decorated = app_user.as_ref().clone();
    decorated.impls.push(Arc::new(CalcitImpl {
      name: EdnTag::new("UserDebug"),
      origin: None,
      fields: Arc::new(vec![]),
      values: Arc::new(vec![]),
    }));
    let decorated_type = CalcitTypeAnnotation::Struct(Arc::new(decorated), Arc::new(vec![]));
    assert_eq!(
      app_type.prove_with_bindings(&decorated_type, &mut TypeBindings::new()),
      TypeProof::Proven,
      "impl decoration must preserve the base data identity"
    );

    let app_ref = CalcitTypeAnnotation::TypeRef(Arc::from("app.models/User"), Arc::new(vec![]));
    let admin_ref = CalcitTypeAnnotation::TypeRef(Arc::from("admin.models/User"), Arc::new(vec![]));
    assert_eq!(app_ref.prove_with_bindings(&app_type, &mut TypeBindings::new()), TypeProof::Proven);
    assert_eq!(
      admin_ref.prove_with_bindings(&app_type, &mut TypeBindings::new()),
      TypeProof::Mismatch
    );
  }

  #[test]
  fn nominal_proof_preserves_applied_arguments_and_rejects_bare_wildcards() {
    let box_def = Arc::new(CalcitStructDef {
      definition_ref: Some(Arc::from("app.models/Box")),
      name: EdnTag::new("Box"),
      fields: Arc::new(vec![EdnTag::new("value")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))]),
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let number = CalcitTypeAnnotation::Struct(box_def.clone(), Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]));
    let string = CalcitTypeAnnotation::Struct(box_def.clone(), Arc::new(vec![Arc::new(CalcitTypeAnnotation::String)]));
    let bare = CalcitTypeAnnotation::Struct(box_def, Arc::new(vec![]));

    assert_eq!(number.prove_with_bindings(&string, &mut TypeBindings::new()), TypeProof::Mismatch);
    assert_ne!(
      number.cmp(&string),
      Ordering::Equal,
      "applied nominal arguments participate in ordering"
    );
    assert_eq!(
      bare.prove_with_bindings(&number, &mut TypeBindings::new()),
      TypeProof::NeedsBoundary(TypeBoundaryReason::ErasedTypeArguments)
    );
  }

  #[test]
  fn nominal_enum_proof_rejects_same_short_name_from_another_namespace() {
    let app_result = Arc::new(generic_result_enum().as_ref().clone().with_definition_ref("app.models", "Result"));
    let admin_result = Arc::new(generic_result_enum().as_ref().clone().with_definition_ref("admin.models", "Result"));
    let app_type = CalcitTypeAnnotation::Enum(
      app_result,
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)]),
    );
    let admin_type = CalcitTypeAnnotation::Enum(
      admin_result,
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)]),
    );

    assert!(!app_type.is_compatible_with(&admin_type));
    assert_eq!(
      app_type.prove_with_bindings(&admin_type, &mut TypeBindings::new()),
      TypeProof::Mismatch
    );
  }

  #[test]
  fn projection_proof_keeps_only_concrete_bindings_across_an_unrelated_boundary() {
    let actual = CalcitTypeAnnotation::TypeRef(
      Arc::from("app/Result"),
      Arc::new(vec![
        Arc::new(CalcitTypeAnnotation::Number),
        Arc::new(CalcitTypeAnnotation::Dynamic),
      ]),
    );
    let expected = CalcitTypeAnnotation::TypeRef(
      Arc::from("app/Result"),
      Arc::new(vec![
        Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T"))),
        Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("E"))),
      ]),
    );
    let mut bindings = TypeBindings::new();
    assert_eq!(
      actual.prove_available_bindings(&expected, &mut bindings),
      TypeProof::NeedsBoundary(TypeBoundaryReason::Dynamic)
    );
    assert!(matches!(bindings.get("T").map(AsRef::as_ref), Some(CalcitTypeAnnotation::Number)));
    assert!(!bindings.contains_key("E"), "Dynamic must not become a concrete generic binding");
  }

  #[test]
  fn proof_requires_a_declared_callable_signature() {
    let signature =
      CalcitTypeAnnotation::from_function_parts(vec![Arc::new(CalcitTypeAnnotation::Number)], Arc::new(CalcitTypeAnnotation::String));
    let mut bindings = TypeBindings::new();
    assert_eq!(
      CalcitTypeAnnotation::DynFn.prove_with_bindings(&signature, &mut bindings),
      TypeProof::NeedsBoundary(TypeBoundaryReason::UnknownCallable)
    );
    assert_eq!(signature.prove_with_bindings(&signature, &mut bindings), TypeProof::Proven);
  }

  #[test]
  fn conflicting_generic_siblings_roll_back_the_whole_projection() {
    let var = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let actual = CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Number));
    let expected = CalcitTypeAnnotation::List(var.clone());
    let mut bindings = TypeBindings::new();
    assert_eq!(actual.prove_available_bindings(&expected, &mut bindings), TypeProof::Proven);
    let committed = bindings.clone();

    let conflicting = CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::String));
    assert_eq!(
      conflicting.prove_available_bindings(&CalcitTypeAnnotation::List(var), &mut bindings),
      TypeProof::Mismatch
    );
    assert_eq!(bindings, committed);
  }

  #[test]
  fn named_enum_satisfies_dynamic_enum_only_in_safe_direction() {
    let named = CalcitTypeAnnotation::Enum(
      generic_result_enum(),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)]),
    );
    let dynamic_enum = CalcitTypeAnnotation::AnonymousEnum;

    assert!(named.is_compatible_with(&dynamic_enum));
    assert!(!dynamic_enum.is_compatible_with(&named));

    // A named reference must never be accepted in the reverse direction. The
    // forward, resolvable TypeRef path is covered by the same matcher arm as
    // the concrete enum assertion above.
    let named_ref = CalcitTypeAnnotation::TypeRef(Arc::from("tests/Result"), Arc::new(vec![]));
    assert!(!dynamic_enum.is_compatible_with(&named_ref));
  }

  #[test]
  fn resolves_enum_definition_through_def_and_impl_traits_wrappers() {
    let enum_form = Calcit::from(vec![
      symbol("defenum"),
      symbol("Result"),
      Calcit::from(vec![Calcit::tag("ok"), symbol("Number")]),
    ]);
    let wrapped = Calcit::from(vec![
      symbol("def"),
      symbol("Result"),
      Calcit::from(vec![symbol("impl-traits"), enum_form, symbol("ResultOpsImpl")]),
    ]);

    assert!(matches!(resolve_type_def_from_code(&wrapped), Some(Calcit::EnumDef(_))));
  }

  #[test]
  fn variadic_function_satisfies_fixed_arity_callback_contract() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let actual = CalcitTypeAnnotation::from_function_parts(
      vec![number.clone(), Arc::new(CalcitTypeAnnotation::Variadic(number.clone()))],
      number.clone(),
    );
    let expected = CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![Arc::from("U"), Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![
        Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("U"))),
        Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T"))),
      ],
      return_type: Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("U"))),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    }));

    assert!(actual.is_compatible_with(&expected));
    assert_eq!(actual.to_brief_string(), "fn(:number, & :number) -> :number");
  }

  #[test]
  fn fixed_arity_function_does_not_satisfy_larger_or_variadic_contract() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let actual = CalcitTypeAnnotation::from_function_parts(vec![number.clone()], number.clone());
    let binary_expected = CalcitTypeAnnotation::from_function_parts(vec![number.clone(), number.clone()], number.clone());
    let variadic_expected = CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![number.clone()],
      return_type: number.clone(),
      fn_kind: SchemaKind::Fn,
      rest_type: Some(number),
      features: Arc::new(HashSet::new()),
    }));

    assert!(!actual.is_compatible_with(&binary_expected));
    assert!(!actual.is_compatible_with(&variadic_expected));
  }

  #[test]
  fn callback_parameter_matching_is_contravariant() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let actual = CalcitTypeAnnotation::from_function_parts(
      vec![Arc::new(CalcitTypeAnnotation::Optional(number.clone()))],
      Arc::new(CalcitTypeAnnotation::Optional(number.clone())),
    );
    let expected = CalcitTypeAnnotation::from_function_parts(vec![number.clone()], Arc::new(CalcitTypeAnnotation::Optional(number)));

    assert!(actual.is_compatible_with(&expected));
  }

  #[test]
  fn js_nullish_and_legacy_optional_are_distinct_boundaries() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let optional = CalcitTypeAnnotation::Optional(number.clone());
    let js_nullish = CalcitTypeAnnotation::JsNullish(number.clone());

    assert!(!js_nullish.is_compatible_with(&optional));
    assert!(!optional.is_compatible_with(&js_nullish));
    assert!(CalcitTypeAnnotation::Number.is_compatible_with(&js_nullish));
    assert!(CalcitTypeAnnotation::Nil.is_compatible_with(&js_nullish));
    assert!(!CalcitTypeAnnotation::Unit.is_compatible_with(&js_nullish));
    assert_eq!(js_nullish.to_type_edn(), Edn::enum_value("JsNullish", vec![number.to_type_edn()]));
  }

  #[test]
  fn nil_and_unit_are_distinct_static_and_runtime_types() {
    assert_eq!(
      CalcitTypeAnnotation::builtin_type_from_tag_name("nil"),
      Some(CalcitTypeAnnotation::Nil)
    );
    assert_eq!(
      CalcitTypeAnnotation::builtin_type_from_tag_name("unit"),
      Some(CalcitTypeAnnotation::Unit)
    );
    assert!(!CalcitTypeAnnotation::Nil.is_compatible_with(&CalcitTypeAnnotation::Unit));
    assert!(!CalcitTypeAnnotation::Unit.is_compatible_with(&CalcitTypeAnnotation::Nil));
    assert!(value_matches_type_annotation(&Calcit::Nil, &CalcitTypeAnnotation::Nil));
    assert!(!value_matches_type_annotation(&Calcit::Nil, &CalcitTypeAnnotation::Unit));
    assert!(value_matches_type_annotation(&Calcit::Unit, &CalcitTypeAnnotation::Unit));
    assert!(!value_matches_type_annotation(&Calcit::Unit, &CalcitTypeAnnotation::Nil));
    assert_eq!(infer_runtime_value_type(&Calcit::Nil).as_ref(), &CalcitTypeAnnotation::Nil);
    assert_eq!(infer_runtime_value_type(&Calcit::Unit).as_ref(), &CalcitTypeAnnotation::Unit);
  }

  #[test]
  fn generic_bindings_lift_nil_and_values_into_optional() {
    let var = Arc::<str>::from("T");
    let mut nil_first = TypeBindings::new();
    assert!(CalcitTypeAnnotation::Nil.compatible_with_bindings(&CalcitTypeAnnotation::TypeVar(var.clone()), &mut nil_first));
    assert!(CalcitTypeAnnotation::Bool.compatible_with_bindings(&CalcitTypeAnnotation::TypeVar(var.clone()), &mut nil_first));
    assert!(matches!(
      nil_first.get(&var).map(Arc::as_ref),
      Some(CalcitTypeAnnotation::Optional(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Bool)
    ));

    let mut value_first = TypeBindings::new();
    assert!(CalcitTypeAnnotation::Bool.compatible_with_bindings(&CalcitTypeAnnotation::TypeVar(var.clone()), &mut value_first));
    assert!(CalcitTypeAnnotation::Nil.compatible_with_bindings(&CalcitTypeAnnotation::TypeVar(var.clone()), &mut value_first));
    assert!(matches!(
      value_first.get(&var).map(Arc::as_ref),
      Some(CalcitTypeAnnotation::Optional(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Bool)
    ));

    assert!(!CalcitTypeAnnotation::Unit.compatible_with_bindings(
      &CalcitTypeAnnotation::Optional(Arc::new(CalcitTypeAnnotation::Bool)),
      &mut TypeBindings::new()
    ));
  }

  #[test]
  fn concrete_struct_matches_struct_category() {
    let actual = CalcitTypeAnnotation::Struct(
      Arc::new(CalcitStructDef::from_fields(EdnTag::new("Person"), vec![EdnTag::new("name")])),
      Arc::new(vec![]),
    );
    let expected = CalcitTypeAnnotation::from_tag_name("struct");

    assert!(actual.is_compatible_with(&expected));
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
      Self::StructValue(struct_def) => {
        "record".hash(state);
        struct_def.name.hash(state);
        struct_def.fields.hash(state);
      }
      Self::EnumValue(enum_def) => {
        "enum-value".hash(state);
        enum_def.name().hash(state);
      }
      Self::AnonymousEnum => "dyntuple".hash(state),
      Self::DynFn => "dynfn".hash(state),
      Self::Fn(signature) => {
        "function".hash(state);
        signature.generics.hash(state);
        signature.arg_types.hash(state);
        signature.return_type.hash(state);
      }
      Self::Macro(signature) => {
        "macro-signature".hash(state);
        signature.hash(state);
      }
      Self::Syntax(contract) => {
        "syntax".hash(state);
        contract.hash(state);
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
      Self::F64Buffer => "f64-buffer".hash(state),
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
      Self::JsNullish(inner) => {
        "js-nullish".hash(state);
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
      Self::StructDef(struct_def) => {
        "struct-def".hash(state);
        struct_def.name.hash(state);
        struct_def.fields.hash(state);
        struct_def.field_types.hash(state);
        struct_def.generics.hash(state);
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
      Self::EnumDef(enum_def) => {
        "enum-def".hash(state);
        enum_def.name().hash(state);
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
      Self::Nil => "nil".hash(state),
      Self::Unit => "unit".hash(state),
      Self::JsObject => "js-object".hash(state),
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
      | (Self::F64Buffer, Self::F64Buffer)
      | (Self::CirruQuote, Self::CirruQuote) => Ordering::Equal,
      (Self::List(a), Self::List(b)) => a.cmp(b),
      (Self::Map(ak, av), Self::Map(bk, bv)) => ak.cmp(bk).then_with(|| av.cmp(bv)),
      (Self::StructValue(a), Self::StructValue(b)) => super::compare::compare_calcit_struct_values(a, b),
      (Self::EnumValue(a), Self::EnumValue(b)) => super::compare::compare_calcit_enum_values(a, b),
      (Self::Fn(a), Self::Fn(b)) => a
        .generics
        .cmp(&b.generics)
        .then_with(|| a.arg_types.cmp(&b.arg_types))
        .then_with(|| a.return_type.cmp(&b.return_type))
        .then_with(|| a.is_async_invocation().cmp(&b.is_async_invocation())),
      (Self::Macro(a), Self::Macro(b)) => a.cmp(b),
      (Self::Syntax(a), Self::Syntax(b)) => a.cmp(b),
      (Self::Set(a), Self::Set(b)) => a.cmp(b),
      (Self::Ref(a), Self::Ref(b)) => a.cmp(b),
      (Self::Variadic(a), Self::Variadic(b)) => a.cmp(b),
      (Self::Custom(a), Self::Custom(b)) => a.cmp(b),
      (Self::Optional(a), Self::Optional(b)) => a.cmp(b),
      (Self::JsNullish(a), Self::JsNullish(b)) => a.cmp(b),
      (Self::Dynamic, Self::Dynamic) => Ordering::Equal,
      (Self::TypeVar(a), Self::TypeVar(b)) => a.cmp(b),
      (Self::TypeRef(a_name, a_args), Self::TypeRef(b_name, b_args)) => a_name.cmp(b_name).then_with(|| a_args.cmp(b_args)),
      (Self::Struct(a, a_args), Self::Struct(b, b_args)) => {
        super::compare::compare_calcit_struct_values(a, b).then_with(|| a_args.cmp(b_args))
      }
      (Self::Enum(a, a_args), Self::Enum(b, b_args)) => {
        super::compare::compare_calcit_enum_values(a, b).then_with(|| a_args.cmp(b_args))
      }
      (Self::StructDef(a), Self::StructDef(b)) => super::compare::compare_calcit_struct_values(a, b),
      (Self::EnumDef(a), Self::EnumDef(b)) => super::compare::compare_calcit_enum_values(a, b),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcitFnTypeAnnotation {
  pub generics: Arc<Vec<Arc<str>>>,
  pub where_bounds: Arc<Vec<CalcitGenericBound>>,
  pub arg_types: Vec<Arc<CalcitTypeAnnotation>>,
  pub return_type: Arc<CalcitTypeAnnotation>,
  /// Whether this schema was declared as `:kind :macro` (default: `:kind :fn`).
  pub fn_kind: SchemaKind,
  /// Rest-param type from `:rest` in the schema, if present.
  pub rest_type: Option<Arc<CalcitTypeAnnotation>>,
  /// Feature flags declared in schema, e.g. `:features $ #{} :js-ffi`.
  pub features: Arc<HashSet<EdnTag>>,
}

impl PartialOrd for CalcitFnTypeAnnotation {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for CalcitFnTypeAnnotation {
  fn cmp(&self, other: &Self) -> Ordering {
    self
      .generics
      .cmp(&other.generics)
      .then_with(|| self.where_bounds.cmp(&other.where_bounds))
      .then_with(|| self.arg_types.cmp(&other.arg_types))
      .then_with(|| self.return_type.cmp(&other.return_type))
      .then_with(|| self.fn_kind.cmp(&other.fn_kind))
      .then_with(|| self.rest_type.cmp(&other.rest_type))
      .then_with(|| self.is_async_invocation().cmp(&other.is_async_invocation()))
  }
}

impl Hash for CalcitFnTypeAnnotation {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.generics.hash(state);
    self.where_bounds.hash(state);
    self.arg_types.hash(state);
    self.return_type.hash(state);
    self.fn_kind.hash(state);
    self.rest_type.hash(state);
    self.is_async_invocation().hash(state);
  }
}

impl CalcitFnTypeAnnotation {
  pub(crate) fn is_async_invocation(&self) -> bool {
    self.features.iter().any(|feature| feature.ref_str() == ASYNC_INVOCATION_FEATURE)
  }

  pub(crate) fn with_async_invocation(&self) -> Self {
    let mut cloned = self.clone();
    let mut features = cloned.features.as_ref().clone();
    features.insert(EdnTag::from(ASYNC_INVOCATION_FEATURE));
    cloned.features = Arc::new(features);
    cloned
  }

  fn where_bounds_to_edn(&self) -> Option<Edn> {
    if self.where_bounds.is_empty() {
      return None;
    }

    let mut map = EdnMapView::default();
    for bound in self.where_bounds.iter() {
      let value = if bound.traits.len() == 1 {
        Edn::Symbol(Arc::from(bound.traits[0].name.ref_str()))
      } else {
        Edn::List(EdnListView(
          bound
            .traits
            .iter()
            .map(|trait_def| Edn::Symbol(Arc::from(trait_def.name.ref_str())))
            .collect(),
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

  fn features_to_edn(&self) -> Option<Edn> {
    let visible_features = self
      .features
      .iter()
      .filter(|feature| feature.ref_str() != ASYNC_INVOCATION_FEATURE)
      .cloned()
      .collect::<Vec<_>>();
    if visible_features.is_empty() {
      return None;
    }
    let mut set = EdnSetView::default();
    for tag in visible_features {
      set.insert(Edn::Tag(tag));
    }
    Some(Edn::Set(set))
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
    if self.is_async_invocation() {
      map.insert_key("async", Edn::Bool(true));
    }
    if let Some(features) = self.features_to_edn() {
      map.insert_key("features", features);
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
    if self.is_async_invocation() {
      map.insert_key("async", Edn::Bool(true));
    }
    if let Some(features) = self.features_to_edn() {
      map.insert_key("features", features);
    }
    Edn::Map(map)
  }

  pub fn to_wrapped_schema_edn(&self) -> Edn {
    let args: Vec<Edn> = self.arg_types.iter().map(|t| t.to_type_edn()).collect();
    let mut map = EdnMapView::default();
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
    if self.is_async_invocation() {
      map.insert_key("async", Edn::Bool(true));
    }
    if let Some(features) = self.features_to_edn() {
      map.insert_key("features", features);
    }

    Edn::enum_value("Fn", vec![Edn::Map(map)])
  }

  pub fn describe(&self) -> String {
    let mut remaining = 128;
    self.describe_bounded(0, &mut remaining)
  }

  fn describe_bounded(&self, depth: usize, remaining: &mut usize) -> String {
    if depth >= TYPE_DIAGNOSTIC_DEPTH_LIMIT || *remaining == 0 {
      return "…".to_owned();
    }
    *remaining -= 1;
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
    let mut rendered_args = vec![];
    for annotation in &self.arg_types {
      if *remaining == 0 {
        rendered_args.push("…".to_owned());
        break;
      }
      rendered_args.push(annotation.describe_bounded(depth + 1, remaining));
    }
    if let Some(rest) = &self.rest_type {
      if *remaining == 0 {
        if rendered_args.last().is_none_or(|part| part != "…") {
          rendered_args.push("…".to_owned());
        }
      } else {
        rendered_args.push(format!("& {}", rest.describe_bounded(depth + 1, remaining)));
      }
    }
    let args = format!("({})", rendered_args.join(", "));
    format!(
      "{}fn{generics}{where_clause}{args} -> {}",
      if self.is_async_invocation() { "async " } else { "" },
      self.return_type.describe_bounded(depth + 1, remaining)
    )
  }

  pub fn render_signature_brief(&self) -> String {
    let mut remaining = 128;
    self.render_signature_brief_bounded(0, &mut remaining)
  }

  fn render_signature_brief_bounded(&self, depth: usize, remaining: &mut usize) -> String {
    if depth >= TYPE_DIAGNOSTIC_DEPTH_LIMIT || *remaining == 0 {
      return "…".to_owned();
    }
    *remaining -= 1;
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
    let mut parts = vec![];
    for annotation in &self.arg_types {
      if *remaining == 0 {
        parts.push("…".to_owned());
        break;
      }
      parts.push(annotation.to_brief_string_bounded(depth + 1, remaining));
    }
    if let Some(rest) = &self.rest_type {
      if *remaining == 0 {
        if parts.last().is_none_or(|part| part != "…") {
          parts.push("…".to_owned());
        }
      } else {
        parts.push(format!("& {}", rest.to_brief_string_bounded(depth + 1, remaining)));
      }
    }
    let args_repr = format!("({})", parts.join(", "));

    format!(
      "{}fn{generics}{where_clause}{args_repr} -> {}",
      if self.is_async_invocation() { "async " } else { "" },
      self.return_type.to_brief_string_bounded(depth + 1, remaining)
    )
  }

  pub fn matches_signature(&self, other: &CalcitFnTypeAnnotation) -> bool {
    let mut bindings = TypeBindings::new();
    self.compatible_signature_with_bindings(other, &mut bindings)
  }

  /// Match a callable signature while preserving bindings from its enclosing
  /// generic call. This lets sibling callbacks share a return type variable,
  /// such as the two branches accepted by `option:fold`.
  pub(crate) fn compatible_signature_with_bindings(&self, other: &CalcitFnTypeAnnotation, bindings: &mut TypeBindings) -> bool {
    if self.is_async_invocation() != other.is_async_invocation() {
      return false;
    }
    // `self` is the actual callable and `other` is the expected callback shape. An actual
    // callable cannot require more fixed arguments than the expected contract guarantees.
    if self.arg_types.len() > other.arg_types.len() {
      return false;
    }

    // Don't require generics count to match: actual concrete functions don't declare
    // generics even when expected fn type uses TypeVars. Bindings resolve TypeVars below.

    let mut staged_bindings = bindings.clone();

    for (idx, expected) in other.arg_types.iter().enumerate() {
      let actual = self.arg_types.get(idx).or(self.rest_type.as_ref());
      let Some(actual) = actual else {
        return false;
      };
      // Callback parameters are contravariant: every value promised by the expected contract
      // must be accepted by the actual callable. This matters for a function accepting
      // `optional<T>` being used where a callback receives `T`.
      if !expected.compatible_with_bindings(actual, &mut staged_bindings) {
        return false;
      }
    }

    // An expected rest contract may invoke the callback with arbitrarily many values, so the
    // actual callable must have a compatible rest parameter as well.
    if let Some(expected_rest) = &other.rest_type {
      let Some(actual_rest) = &self.rest_type else {
        return false;
      };
      if !expected_rest.compatible_with_bindings(actual_rest, &mut staged_bindings) {
        return false;
      }
    }

    if !self
      .return_type
      .compatible_with_bindings(other.return_type.as_ref(), &mut staged_bindings)
    {
      return false;
    }
    *bindings = staged_bindings;
    true
  }

  fn prove_signature_with_bindings(&self, other: &CalcitFnTypeAnnotation, bindings: &mut TypeBindings) -> TypeProof {
    if self.is_async_invocation() != other.is_async_invocation() {
      return TypeProof::Mismatch;
    }
    if self.arg_types.len() > other.arg_types.len() {
      return TypeProof::Mismatch;
    }

    let mut staged = bindings.clone();
    let mut result = TypeProof::Proven;
    for (idx, expected) in other.arg_types.iter().enumerate() {
      let Some(actual) = self.arg_types.get(idx).or(self.rest_type.as_ref()) else {
        return TypeProof::Mismatch;
      };
      result = result.and(expected.prove_with_staged_bindings(actual, &mut staged));
    }
    match (&self.rest_type, &other.rest_type) {
      (Some(actual), Some(expected)) => {
        result = result.and(expected.prove_with_staged_bindings(actual, &mut staged));
      }
      (None, Some(_)) => return TypeProof::Mismatch,
      _ => {}
    }
    result = result.and(self.return_type.prove_with_staged_bindings(&other.return_type, &mut staged));
    if result.is_proven() {
      *bindings = staged;
    }
    result
  }
}

/// Check if a runtime `Calcit` value matches the expected `CalcitTypeAnnotation`.
/// Used for runtime type validation when creating structs (`%{}`) and enums (`%::`).
/// Returns `true` if the value is compatible with the declared type.
/// `Dynamic` types always match. `Nil` matches legacy Optional and JS-nullish boundary types.
pub fn value_matches_type_annotation(value: &Calcit, expected: &CalcitTypeAnnotation) -> bool {
  match expected {
    CalcitTypeAnnotation::Dynamic => true,
    CalcitTypeAnnotation::Nil => matches!(value, Calcit::Nil),
    CalcitTypeAnnotation::Unit => matches!(value, Calcit::Unit),
    CalcitTypeAnnotation::Optional(inner) => matches!(value, Calcit::Nil) || value_matches_type_annotation(value, inner),
    CalcitTypeAnnotation::JsNullish(inner) => matches!(value, Calcit::Nil) || value_matches_type_annotation(value, inner),
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
    CalcitTypeAnnotation::F64Buffer => matches!(value, Calcit::F64Buffer(_)),
    CalcitTypeAnnotation::CirruQuote => matches!(value, Calcit::CirruQuote(_)),
    CalcitTypeAnnotation::AnonymousEnum => matches!(value, Calcit::Enum(_)),
    CalcitTypeAnnotation::DynFn | CalcitTypeAnnotation::Fn(_) => matches!(value, Calcit::Fn { .. } | Calcit::Proc(_)),
    CalcitTypeAnnotation::Macro(_) => matches!(value, Calcit::Macro { .. }),
    CalcitTypeAnnotation::Syntax(_) => false,
    CalcitTypeAnnotation::Struct(expected_struct, _) => match value {
      Calcit::Struct(r) => r.struct_ref.name == expected_struct.name,
      _ => false,
    },
    CalcitTypeAnnotation::Enum(expected_enum, _) => match value {
      Calcit::Enum(t) => t.sum_type.as_ref().is_some_and(|st| st.name() == expected_enum.name()),
      _ => false,
    },
    CalcitTypeAnnotation::TypeRef(expected_name, expected_args) => {
      let nominal_match = match value {
        Calcit::Struct(r) => CalcitTypeAnnotation::type_ref_name_matches(expected_name, r.struct_ref.name.ref_str()),
        Calcit::Enum(t) => t
          .sum_type
          .as_ref()
          .is_some_and(|st| CalcitTypeAnnotation::type_ref_name_matches(expected_name, st.name().ref_str())),
        _ => false,
      };
      if nominal_match {
        true
      } else if expected_args.is_empty()
        && let Some(resolved) = resolve_type_ref_as_schema(expected_name)
      {
        let Ok(_guard) = enter_alias_relation(expected_name, expected, false, false) else {
          return false;
        };
        value_matches_type_annotation(value, resolved.as_ref())
      } else {
        false
      }
    }
    CalcitTypeAnnotation::StructDef(expected_struct) => match value {
      Calcit::StructDef(struct_def) => struct_def.name == expected_struct.name,
      _ => false,
    },
    CalcitTypeAnnotation::EnumDef(expected_enum) => match value {
      Calcit::EnumDef(enum_def) => enum_def.name() == expected_enum.name(),
      _ => false,
    },
    CalcitTypeAnnotation::StructValue(expected_struct) => match value {
      Calcit::Struct(r) => r.struct_ref.name == expected_struct.name,
      _ => false,
    },
    CalcitTypeAnnotation::EnumValue(expected_enum) => match value {
      Calcit::Enum(t) => t.sum_type.as_ref().is_some_and(|st| st.name() == expected_enum.name()),
      _ => false,
    },
    CalcitTypeAnnotation::Trait(expected_trait) => match value {
      Calcit::Struct(r) => r
        .struct_ref
        .impls
        .iter()
        .any(|imp| imp.matches_trait_reference(expected_trait.as_ref())),
      Calcit::Enum(t) => t.impls().iter().any(|imp| imp.matches_trait_reference(expected_trait.as_ref())),
      _ => false,
    },
    CalcitTypeAnnotation::TraitSet(traits) => match value {
      Calcit::Struct(r) => traits
        .iter()
        .all(|trait_def| r.struct_ref.impls.iter().any(|imp| imp.matches_trait_reference(trait_def.as_ref()))),
      Calcit::Enum(t) => traits
        .iter()
        .all(|trait_def| t.impls().iter().any(|imp| imp.matches_trait_reference(trait_def.as_ref()))),
      _ => false,
    },
    CalcitTypeAnnotation::Custom(custom) => match custom.as_ref() {
      Calcit::Tag(tag) => match tag.ref_str() {
        "any" => true,
        "nil" => matches!(value, Calcit::Nil),
        "record" | "struct" => matches!(value, Calcit::Struct(_)),
        "tuple" | "enum" => matches!(value, Calcit::Enum(_)),
        "struct-def" => matches!(value, Calcit::StructDef(_)),
        "enum-def" => matches!(value, Calcit::EnumDef(_)),
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
    CalcitTypeAnnotation::JsObject => true, // opaque external data, allow any
  }
}

fn infer_runtime_struct_applied_args(struct_def: &CalcitStructDef, values: &[Calcit]) -> Vec<Arc<CalcitTypeAnnotation>> {
  if struct_def.generics.is_empty() {
    return vec![];
  }

  let mut bindings: TypeBindings = HashMap::new();
  for (value, expected_type) in values.iter().zip(struct_def.field_types.iter()) {
    collect_runtime_type_bindings(value, expected_type.as_ref(), &mut bindings);
  }

  struct_def
    .generics
    .iter()
    .map(|name| bindings.get(name).cloned().unwrap_or_else(|| crate::calcit::DYNAMIC_TYPE.clone()))
    .collect()
}

fn infer_runtime_enum_applied_args(enum_def: &CalcitEnumDef, enum_value: &CalcitEnumValue) -> Vec<Arc<CalcitTypeAnnotation>> {
  if enum_def.generics().is_empty() {
    return vec![];
  }

  let Some(Calcit::Tag(tag)) = Some(enum_value.tag.as_ref()) else {
    return enum_def.generics().iter().map(|_| crate::calcit::DYNAMIC_TYPE.clone()).collect();
  };
  let Some(variant) = enum_def.find_variant(tag) else {
    return enum_def.generics().iter().map(|_| crate::calcit::DYNAMIC_TYPE.clone()).collect();
  };

  let mut bindings: TypeBindings = HashMap::new();
  for (value, expected_type) in enum_value.extra.iter().zip(variant.payload_types().iter()) {
    collect_runtime_type_bindings(value, expected_type.as_ref(), &mut bindings);
  }

  enum_def
    .generics()
    .iter()
    .map(|name| bindings.get(name).cloned().unwrap_or_else(|| crate::calcit::DYNAMIC_TYPE.clone()))
    .collect()
}

pub fn infer_runtime_value_type(value: &Calcit) -> Arc<CalcitTypeAnnotation> {
  match value {
    Calcit::Nil => Arc::new(CalcitTypeAnnotation::Nil),
    Calcit::Unit => Arc::new(CalcitTypeAnnotation::Unit),
    Calcit::Bool(_) => Arc::new(CalcitTypeAnnotation::Bool),
    Calcit::Number(_) => Arc::new(CalcitTypeAnnotation::Number),
    Calcit::Str(_) => Arc::new(CalcitTypeAnnotation::String),
    Calcit::Symbol { .. } => Arc::new(CalcitTypeAnnotation::Symbol),
    Calcit::Tag(_) => Arc::new(CalcitTypeAnnotation::Tag),
    Calcit::List(_) => Arc::new(CalcitTypeAnnotation::List(crate::calcit::DYNAMIC_TYPE.clone())),
    Calcit::Map(_) => Arc::new(CalcitTypeAnnotation::Map(
      crate::calcit::DYNAMIC_TYPE.clone(),
      crate::calcit::DYNAMIC_TYPE.clone(),
    )),
    Calcit::Set(_) => Arc::new(CalcitTypeAnnotation::Set(crate::calcit::DYNAMIC_TYPE.clone())),
    Calcit::Ref(..) => Arc::new(CalcitTypeAnnotation::Ref(crate::calcit::DYNAMIC_TYPE.clone())),
    Calcit::Buffer(_) => Arc::new(CalcitTypeAnnotation::Buffer),
    Calcit::F64Buffer(_) => Arc::new(CalcitTypeAnnotation::F64Buffer),
    Calcit::CirruQuote(_) => Arc::new(CalcitTypeAnnotation::CirruQuote),
    Calcit::Fn { info, .. } => Arc::new(CalcitTypeAnnotation::from_calcit_fn(info)),
    Calcit::Proc(proc) => proc
      .get_type_signature()
      .map(|signature| {
        Arc::new(CalcitTypeAnnotation::from_function_parts(
          signature.arg_types.clone(),
          signature.return_type.clone(),
        ))
      })
      .unwrap_or_else(|| Arc::new(CalcitTypeAnnotation::DynFn)),
    Calcit::Struct(struct_value) => {
      if struct_value.struct_ref.generics.is_empty() {
        Arc::new(CalcitTypeAnnotation::StructValue(struct_value.struct_ref.clone()))
      } else {
        Arc::new(CalcitTypeAnnotation::Struct(
          struct_value.struct_ref.clone(),
          Arc::new(infer_runtime_struct_applied_args(
            struct_value.struct_ref.as_ref(),
            struct_value.values.as_ref(),
          )),
        ))
      }
    }
    Calcit::StructDef(struct_def) => Arc::new(CalcitTypeAnnotation::StructDef(Arc::new(struct_def.to_owned()))),
    Calcit::Enum(enum_value) => match &enum_value.sum_type {
      Some(enum_def) if enum_def.generics().is_empty() => Arc::new(CalcitTypeAnnotation::EnumValue(enum_def.clone())),
      Some(enum_def) => Arc::new(CalcitTypeAnnotation::Enum(
        enum_def.clone(),
        Arc::new(infer_runtime_enum_applied_args(enum_def.as_ref(), enum_value)),
      )),
      None => Arc::new(CalcitTypeAnnotation::AnonymousEnum),
    },
    Calcit::EnumDef(enum_def) => Arc::new(CalcitTypeAnnotation::EnumDef(Arc::new(enum_def.to_owned()))),
    _ => Arc::new(CalcitTypeAnnotation::from_calcit(value)),
  }
}

pub fn collect_runtime_type_bindings(value: &Calcit, expected: &CalcitTypeAnnotation, bindings: &mut TypeBindings) -> bool {
  let actual = infer_runtime_value_type(value);
  actual.as_ref().compatible_with_bindings(expected, bindings)
}

pub fn validate_runtime_generic_where_bounds(bindings: &TypeBindings, where_bounds: &[CalcitGenericBound]) -> Result<(), String> {
  for bound in where_bounds {
    let Some(actual_type) = bindings.get(&bound.name) else {
      continue;
    };
    if matches!(actual_type.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn) {
      continue;
    }

    let required = bound.as_type_annotation();
    if actual_type.as_ref().is_compatible_with(required.as_ref()) {
      continue;
    }

    return Err(format!(
      "generic '{}' is bound to `{}`, but it does not satisfy `{}`",
      bound.name,
      actual_type.to_brief_string(),
      required.to_brief_string()
    ));
  }

  Ok(())
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
    Calcit::F64Buffer(_) => "f64-buffer",
    Calcit::CirruQuote(_) => "cirru-quote",
    Calcit::Enum(_) => "enum",
    Calcit::Struct(_) => "struct",
    Calcit::StructDef(_) => "struct-def",
    Calcit::EnumDef(_) => "enum-def",
    Calcit::Fn { .. } | Calcit::Proc(_) => "fn",
    Calcit::Macro { .. } => "macro",
    Calcit::Syntax(..) => "syntax",
    Calcit::Method(..) => "method",
    Calcit::Trait(_) => "trait",
    Calcit::Impl(_) => "impl",
    _ => "unknown",
  }
}
