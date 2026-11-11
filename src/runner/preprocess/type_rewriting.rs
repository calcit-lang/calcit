//! Type-directed rewriting module.
//!
//! Rewrites untyped AST literals to typed forms when the **expected type**
//! (from function parameter annotations) is known. This is the "checking
//! direction" applied structurally: we know the target type and rewrite
//! the argument to match.
//!
//! Three families of rewrites:
//! 1. Map literal `{} (:k v)` → Struct literal `%{} Struct (:k v)` when param is struct-typed
//! 2. Loose struct `?{} :k v` → Struct literal `%{} Struct :k v` when param is struct-typed
//!
//! In families 1 and 2 a plain `T` payload for an `Option<T>` field is wrapped
//! as `Option :some` and an omitted Option field becomes `Option :none` (#2652).
//! 3. Enum literal `:: :tag ...` → Named enum `%:: Enum :tag ...` when param is enum-typed

use std::cell::RefCell;
use std::sync::Arc;

use cirru_edn::EdnTag;

use crate::calcit::{
  Calcit, CalcitEnumDef, CalcitFn, CalcitFnTypeAnnotation, CalcitImport, CalcitList, CalcitProc, CalcitStructDef, CalcitTypeAnnotation,
  ImportInfo, LocatedWarning,
};

use super::{ScopeTypes, core_import, gen_check_warning, lift_option_field_payload};

// ---------------------------------------------------------------------------
// Unified argument rewriting loop
// ---------------------------------------------------------------------------

/// Generic argument-list rewriter. Walks `processed_args` positionally against
/// `arg_types`, calling `rewrite_single` for each pair. Returns a new CalcitList
/// only when at least one argument was actually rewritten.
fn rewrite_args_by_expected_type<F>(
  arg_types: &[Arc<CalcitTypeAnnotation>],
  processed_args: &CalcitList,
  mut rewrite_single: F,
) -> Option<CalcitList>
where
  F: FnMut(&Calcit, &Arc<CalcitTypeAnnotation>, usize) -> Option<Calcit>,
{
  if arg_types.is_empty() {
    return None;
  }

  let mut rewritten = false;
  let mut new_args: Vec<Calcit> = Vec::with_capacity(processed_args.len());

  for (idx, arg) in processed_args.iter().enumerate() {
    if let Some(expected) = arg_types.get(idx)
      && let Some(rewritten_arg) = rewrite_single(arg, expected, idx)
    {
      new_args.push(rewritten_arg);
      rewritten = true;
      continue;
    }
    new_args.push(arg.to_owned());
  }

  if rewritten {
    Some(CalcitList::from(new_args.as_slice()))
  } else {
    None
  }
}

// ---------------------------------------------------------------------------
// Public entry points (called from preprocess_list_call)
// ---------------------------------------------------------------------------

/// Rewrite untyped enum literal args to named enums for local function calls.
pub(crate) fn try_rewrite_local_fn_enum_args_to_named_enums(
  fn_annot: &CalcitFnTypeAnnotation,
  local_name: &str,
  processed_args: &CalcitList,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Option<CalcitList> {
  rewrite_args_by_expected_type(&fn_annot.arg_types, processed_args, |arg, expected, idx| {
    try_rewrite_single_enum_to_named_enum(arg, expected, file_ns, def_name, local_name, idx, check_warnings)
  })
}

/// Rewrite hashmap literal arguments to struct literals when struct-typed.
pub(crate) fn try_rewrite_map_args_to_structs(
  fn_info: &CalcitFn,
  expected_types: Option<&[Arc<CalcitTypeAnnotation>]>,
  processed_args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Option<CalcitList> {
  rewrite_args_by_expected_type(
    expected_types.unwrap_or(&fn_info.arg_types),
    processed_args,
    |arg, expected, idx| {
      try_rewrite_single_map_to_struct(arg, expected, scope_types, file_ns, def_name, &fn_info.name, idx, check_warnings)
    },
  )
}

/// Rewrite loose struct literal arguments to struct literals when struct-typed.
pub(crate) fn try_rewrite_loose_struct_args_to_structs(
  fn_info: &CalcitFn,
  expected_types: Option<&[Arc<CalcitTypeAnnotation>]>,
  processed_args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Option<CalcitList> {
  rewrite_args_by_expected_type(
    expected_types.unwrap_or(&fn_info.arg_types),
    processed_args,
    |arg, expected, idx| {
      try_rewrite_single_loose_struct_to_struct(arg, expected, scope_types, file_ns, def_name, &fn_info.name, idx, check_warnings)
    },
  )
}

/// Rewrite untyped enum literal arguments to named enums when enum-typed.
pub(crate) fn try_rewrite_enum_args_to_named_enums(
  fn_info: &CalcitFn,
  expected_types: Option<&[Arc<CalcitTypeAnnotation>]>,
  processed_args: &CalcitList,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Option<CalcitList> {
  rewrite_args_by_expected_type(
    expected_types.unwrap_or(&fn_info.arg_types),
    processed_args,
    |arg, expected, idx| try_rewrite_single_enum_to_named_enum(arg, expected, file_ns, def_name, &fn_info.name, idx, check_warnings),
  )
}

// ---------------------------------------------------------------------------
// Single-argument rewrite implementations
// ---------------------------------------------------------------------------

/// Value for a struct field the literal leaves out, matching the direct
/// constructor: a declared default comes first, nominal Option fields get the
/// lowered `Option :none`, other fields keep nil. Arguments are already
/// preprocessed, so emit the lowered form.
fn omitted_struct_field_value(struct_def: &CalcitStructDef, field_idx: usize, file_ns: &str) -> Calcit {
  if let Some(default_value) = struct_def.field_default(field_idx) {
    crate::calcit::struct_field_default_code(default_value)
  } else if struct_def
    .field_types
    .get(field_idx)
    .is_some_and(|field_type| field_type.is_option_type())
  {
    Calcit::from(vec![
      Calcit::Proc(CalcitProc::NativeNamedEnumNew),
      core_import("Option", file_ns),
      Calcit::tag("none"),
    ])
  } else {
    Calcit::Nil
  }
}

/// Try to rewrite a single hashmap literal to a struct literal if the expected type is a struct.
#[allow(clippy::too_many_arguments)]
fn try_rewrite_single_map_to_struct(
  arg: &Calcit,
  expected_type: &Arc<CalcitTypeAnnotation>,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  fn_name: &str,
  arg_idx: usize,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Option<Calcit> {
  // Only handle hashmap literals (lists starting with Proc(NativeMap))
  let Calcit::List(arg_list) = arg else { return None };
  let Some(Calcit::Proc(CalcitProc::NativeMap)) = arg_list.first() else {
    return None;
  };

  // Resolve the expected type to a struct definition + optional ns/def path
  let (struct_def, ns_def_path) = expected_type.resolve_to_struct_with_ref()?;

  // Validate: the map literal has flat key-value pairs after the NativeMap head
  let map_items: Vec<&Calcit> = arg_list.iter().skip(1).collect();
  if !map_items.len().is_multiple_of(2) {
    return None; // malformed map literal, skip
  }

  // Validate all keys are tags AND are valid struct fields
  let mut provided_fields: std::collections::HashMap<EdnTag, &Calcit> = std::collections::HashMap::new();
  for chunk in map_items.chunks(2) {
    if let Calcit::Tag(key) = &chunk[0] {
      if !struct_def.fields.iter().any(|f| f == key) {
        gen_check_warning(
          format!(
            "[Warn] map-to-struct rewrite skipped for `{fn_name}` arg {}: key `:{key}` is not a field of struct, at {file_ns}/{def_name}",
            arg_idx + 1,
          ),
          file_ns,
          check_warnings,
        );
        return None;
      }
      provided_fields.insert(key.to_owned(), chunk[1]);
    } else {
      gen_check_warning(
        format!(
          "[Warn] map-to-struct rewrite skipped for `{fn_name}` arg {}: non-tag key `{}` found, at {file_ns}/{def_name}",
          arg_idx + 1,
          chunk[0]
        ),
        file_ns,
        check_warnings,
      );
      return None;
    }
  }

  let struct_ref_node = build_struct_ref_node(&struct_def, ns_def_path, file_ns, def_name);

  // Build the rewritten struct literal with ALL struct fields in order.
  let mut struct_items: Vec<Calcit> = Vec::with_capacity(struct_def.fields.len() * 2 + 2);
  struct_items.push(Calcit::Proc(CalcitProc::NativeStruct));
  struct_items.push(struct_ref_node);
  for (field_idx, field) in struct_def.fields.iter().enumerate() {
    struct_items.push(Calcit::Tag(field.to_owned()));
    if let Some(value) = provided_fields.get(field) {
      // A plain `T` for an `Option<T>` field is wrapped as `Option :some` (#2652).
      let lifted = struct_def
        .field_types
        .get(field_idx)
        .and_then(|field_type| lift_option_field_payload(value, field_type, &struct_def.generics, scope_types, file_ns));
      // A bare tag for an enum-typed field names its zero-payload variant (#2701);
      // other tag outcomes are reported by the constructor field check.
      let lifted = lifted.or_else(|| {
        let Calcit::Tag(tag) = value else { return None };
        let field_type = struct_def.field_types.get(field_idx)?;
        match classify_tag_for_expected(tag, field_type, file_ns, def_name)? {
          TagCoercion::Rewrite(form) => Some(form),
          _ => None,
        }
      });
      struct_items.push(lifted.unwrap_or_else(|| (*value).to_owned()));
    } else {
      struct_items.push(omitted_struct_field_value(&struct_def, field_idx, file_ns));
    }
  }

  Some(Calcit::from(struct_items))
}

/// Try to rewrite a single loose struct literal (`?{} :field val ...`) to a struct literal.
#[allow(clippy::too_many_arguments)]
fn try_rewrite_single_loose_struct_to_struct(
  arg: &Calcit,
  expected_type: &Arc<CalcitTypeAnnotation>,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  fn_name: &str,
  arg_idx: usize,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Option<Calcit> {
  // Only handle loose struct literals (lists starting with Proc(NativeLooseStruct))
  let Calcit::List(arg_list) = arg else { return None };
  let Some(Calcit::Proc(CalcitProc::NativeLooseStruct)) = arg_list.first() else {
    return None;
  };

  // Resolve the expected type to a struct definition + optional ns/def path
  let (struct_def, ns_def_path) = expected_type.resolve_to_struct_with_ref()?;

  // Validate: the loose struct literal has flat key-value pairs after the NativeLooseStruct head
  let items: Vec<&Calcit> = arg_list.iter().skip(1).collect();
  if !items.len().is_multiple_of(2) {
    return None; // malformed, skip
  }

  // Validate all keys are tags AND are valid struct fields
  let mut provided_fields: std::collections::HashMap<cirru_edn::EdnTag, &Calcit> = std::collections::HashMap::new();
  for chunk in items.chunks(2) {
    if let Calcit::Tag(key) = &chunk[0] {
      if !struct_def.fields.iter().any(|f| f == key) {
        gen_check_warning(
          format!(
            "[Warn] loose-struct-to-struct rewrite skipped for `{fn_name}` arg {}: key `:{key}` is not a field of struct `{}`, at {file_ns}/{def_name}",
            arg_idx + 1,
            struct_def.name,
          ),
          file_ns,
          check_warnings,
        );
        return None;
      }
      provided_fields.insert(key.to_owned(), chunk[1]);
    } else {
      gen_check_warning(
        format!(
          "[Warn] loose-struct-to-struct rewrite skipped for `{fn_name}` arg {}: non-tag key `{}` found, at {file_ns}/{def_name}",
          arg_idx + 1,
          chunk[0]
        ),
        file_ns,
        check_warnings,
      );
      return None;
    }
  }

  let struct_ref_node = build_struct_ref_node(&struct_def, ns_def_path, file_ns, def_name);

  // Build the rewritten struct literal with ALL struct fields in order.
  let mut struct_items: Vec<Calcit> = Vec::with_capacity(struct_def.fields.len() * 2 + 2);
  struct_items.push(Calcit::Proc(CalcitProc::NativeStruct));
  struct_items.push(struct_ref_node);
  for (field_idx, field) in struct_def.fields.iter().enumerate() {
    struct_items.push(Calcit::Tag(field.to_owned()));
    if let Some(value) = provided_fields.get(field) {
      // A plain `T` for an `Option<T>` field is wrapped as `Option :some` (#2652).
      let lifted = struct_def
        .field_types
        .get(field_idx)
        .and_then(|field_type| lift_option_field_payload(value, field_type, &struct_def.generics, scope_types, file_ns));
      // A bare tag for an enum-typed field names its zero-payload variant (#2701);
      // other tag outcomes are reported by the constructor field check.
      let lifted = lifted.or_else(|| {
        let Calcit::Tag(tag) = value else { return None };
        let field_type = struct_def.field_types.get(field_idx)?;
        match classify_tag_for_expected(tag, field_type, file_ns, def_name)? {
          TagCoercion::Rewrite(form) => Some(form),
          _ => None,
        }
      });
      struct_items.push(lifted.unwrap_or_else(|| (*value).to_owned()));
    } else {
      struct_items.push(omitted_struct_field_value(&struct_def, field_idx, file_ns));
    }
  }

  Some(Calcit::from(struct_items))
}

/// Try to rewrite a single untyped enum literal (`:: :tag payload...`) to a named enum
/// (`%:: EnumDef :tag payload...`) if the expected type is an enum.
fn try_rewrite_single_enum_to_named_enum(
  arg: &Calcit,
  expected_type: &Arc<CalcitTypeAnnotation>,
  file_ns: &str,
  def_name: &str,
  fn_name: &str,
  arg_idx: usize,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Option<Calcit> {
  // Only handle untyped enum literals (lists starting with Proc(NativeEnum))
  let Calcit::List(arg_list) = arg else { return None };
  let Some(Calcit::Proc(CalcitProc::NativeEnum)) = arg_list.first() else {
    return None;
  };

  // Resolve the expected type to an enum definition + optional ns/def path
  let (enum_def, ns_def_path) = expected_type.resolve_to_enum_with_ref()?;

  // Validate: the enum literal needs at least a tag
  if arg_list.len() < 2 {
    gen_check_warning(
      format!(
        "[Warn] enum-to-named-enum rewrite skipped for `{fn_name}` arg {}: enum literal has no tag, at {file_ns}/{def_name}",
        arg_idx + 1,
      ),
      file_ns,
      check_warnings,
    );
    return None;
  }

  // Validate: check that the tag is a known variant of the enum
  if let Some(Calcit::Tag(tag)) = arg_list.get(1) {
    let tag_str = tag.ref_str();
    let variants = enum_def.variants();
    if !variants.iter().any(|v| v.tag.ref_str() == tag_str) {
      let variant_names: Vec<&str> = variants.iter().map(|v| v.tag.ref_str()).collect();
      gen_check_warning(
        format!(
          "[Warn] Enum `{}` does not have variant `:{tag_str}`. Available variants: {variant_names:?}, at {file_ns}/{def_name}",
          enum_def.name(),
        ),
        file_ns,
        check_warnings,
      );
    }
  }

  let enum_ref_node = build_enum_ref_node(enum_def, ns_def_path, file_ns, def_name);

  // Build rewritten: [NativeNamedEnumNew, enum_ref, tag, ...payloads]
  let mut items: Vec<Calcit> = Vec::with_capacity(arg_list.len() + 1);
  items.push(Calcit::Proc(CalcitProc::NativeNamedEnumNew));
  items.push(enum_ref_node);
  // Copy tag and payloads from the original enum literal (skip NativeEnum head)
  for item in arg_list.iter().skip(1) {
    items.push(item.to_owned());
  }

  Some(Calcit::from(items))
}

// ---------------------------------------------------------------------------
// Shared reference node builders
// ---------------------------------------------------------------------------

/// Build a Calcit node referencing a struct definition, using Import when ns/def is known.
pub(crate) fn build_struct_ref_node(
  struct_def: &CalcitStructDef,
  ns_def_path: Option<(Arc<str>, Arc<str>)>,
  file_ns: &str,
  def_name: &str,
) -> Calcit {
  if let Some((ns, def)) = ns_def_path {
    let import_info = if ns.as_ref() == file_ns {
      ImportInfo::SameFile {
        at_def: Arc::from(def_name),
      }
    } else {
      ImportInfo::NsReferDef {
        at_ns: Arc::from(file_ns),
        at_def: Arc::from(def_name),
      }
    };
    Calcit::Import(CalcitImport {
      ns: ns.to_owned(),
      def: def.to_owned(),
      info: Arc::new(import_info),
      def_id: None,
    })
  } else {
    Calcit::StructDef(struct_def.clone())
  }
}

/// Build a Calcit node referencing an enum definition, using Import when ns/def is known.
pub(crate) fn build_enum_ref_node(
  enum_def: CalcitEnumDef,
  ns_def_path: Option<(Arc<str>, Arc<str>)>,
  file_ns: &str,
  def_name: &str,
) -> Calcit {
  if let Some((ns, def)) = ns_def_path {
    let import_info = if ns.as_ref() == file_ns {
      ImportInfo::SameFile {
        at_def: Arc::from(def_name),
      }
    } else {
      ImportInfo::NsReferDef {
        at_ns: Arc::from(file_ns),
        at_def: Arc::from(def_name),
      }
    };
    Calcit::Import(CalcitImport {
      ns: ns.to_owned(),
      def: def.to_owned(),
      info: Arc::new(import_info),
      def_id: None,
    })
  } else {
    Calcit::Struct(enum_def.to_struct_prototype())
  }
}

// ---------------------------------------------------------------------------
// Bare tag literal → zero-payload enum variant
// ---------------------------------------------------------------------------

/// What a bare tag literal means at a position whose expected type is known.
pub(crate) enum TagCoercion {
  /// The tag names exactly one zero-payload variant: emit `%:: Enum :tag`.
  Rewrite(Calcit),
  /// The expected enum has no variant with this tag.
  UnknownVariant { enum_name: String, variants: Vec<String> },
  /// The variant exists but carries payloads, so a bare tag cannot build it.
  NeedsPayload { enum_name: String, arity: usize },
  /// Several enum members of a union have a zero-payload variant with this tag.
  Ambiguous { candidates: Vec<String> },
}

fn enum_display_name(enum_def: &CalcitEnumDef) -> String {
  enum_def
    .definition_ref()
    .map(|reference| reference.rsplit_once('/').map(|(_, def)| def).unwrap_or(reference).to_owned())
    .unwrap_or_else(|| enum_def.name().ref_str().to_owned())
}

/// Core `Option` and `Result` keep their explicit constructors: absence and
/// failure must stay visible at the call site.
fn is_core_wrapper_enum(enum_def: &CalcitEnumDef) -> bool {
  let reference = enum_def
    .definition_ref()
    .map(|reference| reference.to_string())
    .unwrap_or_else(|| enum_def.name().ref_str().to_owned());
  matches!(
    reference.as_str(),
    "calcit.core/Option" | "calcit.core/Result" | "Option" | "Result"
  )
}

fn zero_payload_constructor(
  enum_def: CalcitEnumDef,
  ns_def_path: Option<(Arc<str>, Arc<str>)>,
  tag: &EdnTag,
  file_ns: &str,
  def_name: &str,
) -> Calcit {
  Calcit::from(vec![
    Calcit::Proc(CalcitProc::NativeNamedEnumNew),
    build_enum_ref_node(enum_def, ns_def_path, file_ns, def_name),
    Calcit::Tag(tag.to_owned()),
  ])
}

/// Decide whether a bare tag literal stands for a zero-payload variant of the
/// expected type (C6 / F5). Only a definite enum, or a `defunion` without a
/// `Tag` member, gives a tag a variant meaning; `Dynamic`, `Tag`, type
/// variables and everything else keep the plain tag (`None`).
pub(crate) fn classify_tag_for_expected(
  tag: &EdnTag,
  expected: &CalcitTypeAnnotation,
  file_ns: &str,
  def_name: &str,
) -> Option<TagCoercion> {
  if expected.contains_type_var() {
    return None;
  }
  if let Some((enum_def, ns_def_path)) = expected.resolve_to_enum_with_ref() {
    if is_core_wrapper_enum(&enum_def) {
      return None;
    }
    let enum_name = enum_display_name(&enum_def);
    return Some(match enum_def.find_variant(tag) {
      Some(variant) if variant.arity() == 0 => {
        TagCoercion::Rewrite(zero_payload_constructor(enum_def, ns_def_path, tag, file_ns, def_name))
      }
      Some(variant) => TagCoercion::NeedsPayload {
        enum_name,
        arity: variant.arity(),
      },
      None => TagCoercion::UnknownVariant {
        enum_name,
        variants: {
          // Sorted, since some resolution paths do not keep declaration order.
          let mut names: Vec<String> = enum_def.variants().iter().map(|v| format!(":{}", v.tag)).collect();
          names.sort();
          names
        },
      },
    });
  }
  let union_def = expected.resolve_to_union()?;
  let members = union_def.checked_members().ok()?;
  if members
    .iter()
    .any(|member| member.kind == crate::calcit::union_def::UnionMemberKind::Tag)
  {
    return None;
  }
  let mut matches = vec![];
  for member in &members {
    if let Some((enum_def, ns_def_path)) = member.annotation.resolve_to_enum_with_ref()
      && !is_core_wrapper_enum(&enum_def)
      && enum_def.find_variant(tag).is_some_and(|variant| variant.arity() == 0)
    {
      matches.push((enum_def, ns_def_path.or_else(|| member.definition.clone())));
    }
  }
  match matches.len() {
    0 => None,
    1 => {
      let (enum_def, ns_def_path) = matches.pop().expect("one match");
      Some(TagCoercion::Rewrite(zero_payload_constructor(
        enum_def,
        ns_def_path,
        tag,
        file_ns,
        def_name,
      )))
    }
    _ => Some(TagCoercion::Ambiguous {
      candidates: matches.iter().map(|(enum_def, _)| enum_display_name(enum_def)).collect(),
    }),
  }
}

/// Rewrite a bare tag literal at a position typed `expected`, or reject it
/// when it cannot name a zero-payload variant there. `position` describes the
/// slot for the message, e.g. "arg 1 of `show-filter`".
pub(crate) fn coerce_tag_literal(
  value: &Calcit,
  expected: &CalcitTypeAnnotation,
  position: &str,
  (file_ns, def_name): (&str, &str),
  call_stack: &crate::call_stack::CallStackList,
  location: Option<crate::calcit::NodeLocation>,
) -> Result<Option<Calcit>, crate::calcit::CalcitErr> {
  let Calcit::Tag(tag) = value else { return Ok(None) };
  let Some(coercion) = classify_tag_for_expected(tag, expected, file_ns, def_name) else {
    return Ok(None);
  };
  let (code, msg) = match coercion {
    TagCoercion::Rewrite(form) => return Ok(Some(form)),
    TagCoercion::UnknownVariant { enum_name, variants } => (
      "E_ENUM_TAG_UNKNOWN",
      format!(
        "{enum_name} has no `:{tag}` variant ({position}, at {file_ns}/{def_name}); available variants: {}",
        variants.join(" ")
      ),
    ),
    TagCoercion::NeedsPayload { enum_name, arity } => (
      "E_ENUM_TAG_NEEDS_PAYLOAD",
      format!(
        "{enum_name} variant `:{tag}` takes {arity} payload(s), so a bare tag cannot build it ({position}, at {file_ns}/{def_name}); write `{enum_name} :{tag} …`"
      ),
    ),
    TagCoercion::Ambiguous { candidates } => (
      "E_ENUM_TAG_AMBIGUOUS",
      format!(
        "bare tag `:{tag}` is a zero-payload variant of several union members ({}) ({position}, at {file_ns}/{def_name}); write the constructor, e.g. `{} :{tag}`",
        candidates.join(", "),
        candidates[0]
      ),
    ),
  };
  Err(crate::calcit::CalcitErr::use_msg_stack_location_with_code(
    crate::calcit::CalcitErrKind::Type,
    msg,
    code,
    call_stack,
    location,
  ))
}

/// [`coerce_tag_literal`] for a function's returned form: the tag may sit
/// directly in tail position or in the branches of a tail `if`.
pub(crate) fn coerce_tail_tag_literals(
  form: &Calcit,
  expected: &CalcitTypeAnnotation,
  position: &str,
  ctx: (&str, &str),
  call_stack: &crate::call_stack::CallStackList,
  location: Option<crate::calcit::NodeLocation>,
) -> Result<Option<Calcit>, crate::calcit::CalcitErr> {
  match form {
    Calcit::Tag(_) => coerce_tag_literal(form, expected, position, ctx, call_stack, location),
    Calcit::List(items)
      if matches!(items.first(), Some(Calcit::Syntax(crate::calcit::CalcitSyntax::If, _))) && matches!(items.len(), 3 | 4) =>
    {
      let mut changed = false;
      let mut next = Vec::with_capacity(items.len());
      for (idx, item) in items.iter().enumerate() {
        let replacement = if idx >= 2 {
          coerce_tail_tag_literals(item, expected, position, ctx, call_stack, location.clone())?
        } else {
          None
        };
        changed |= replacement.is_some();
        next.push(replacement.unwrap_or_else(|| item.to_owned()));
      }
      Ok(changed.then(|| Calcit::from(next)))
    }
    _ => Ok(None),
  }
}
