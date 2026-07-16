//! Type-directed rewriting module.
//!
//! Rewrites untyped AST literals to typed forms when the **expected type**
//! (from function parameter annotations) is known. This is the "checking
//! direction" applied structurally: we know the target type and rewrite
//! the argument to match.
//!
//! Three families of rewrites:
//! 1. Map literal `{} (:k v)` → Record literal `%{} Struct (:k v)` when param is struct-typed
//! 2. Loose record `?{} :k v` → Record literal `%{} Struct :k v` when param is struct-typed
//! 3. Tuple literal `:: :tag ...` → Enum tuple `%:: Enum :tag ...` when param is enum-typed

use std::cell::RefCell;
use std::sync::Arc;

use cirru_edn::EdnTag;

use crate::calcit::{
  Calcit, CalcitEnum, CalcitFn, CalcitFnTypeAnnotation, CalcitImport, CalcitList, CalcitProc, CalcitStruct, CalcitTypeAnnotation,
  ImportInfo, LocatedWarning,
};

use super::gen_check_warning;

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
      && let Some(rewritten_arg) = rewrite_single(arg, expected, idx) {
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

/// Rewrite untyped tuple literal args to enum tuples for local function calls.
pub(crate) fn try_rewrite_local_fn_tuple_args_to_enum_tuples(
  fn_annot: &CalcitFnTypeAnnotation,
  local_name: &str,
  processed_args: &CalcitList,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Option<CalcitList> {
  rewrite_args_by_expected_type(&fn_annot.arg_types, processed_args, |arg, expected, idx| {
    try_rewrite_single_tuple_to_enum_tuple(arg, expected, file_ns, def_name, local_name, idx, check_warnings)
  })
}

/// Rewrite hashmap literal arguments to record literals when struct-typed.
pub(crate) fn try_rewrite_map_args_to_records(
  fn_info: &CalcitFn,
  processed_args: &CalcitList,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Option<CalcitList> {
  rewrite_args_by_expected_type(&fn_info.arg_types, processed_args, |arg, expected, idx| {
    try_rewrite_single_map_to_record(arg, expected, file_ns, def_name, &fn_info.name, idx, check_warnings)
  })
}

/// Rewrite loose record literal arguments to struct record literals when struct-typed.
pub(crate) fn try_rewrite_loose_record_args_to_struct_records(
  fn_info: &CalcitFn,
  processed_args: &CalcitList,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Option<CalcitList> {
  rewrite_args_by_expected_type(&fn_info.arg_types, processed_args, |arg, expected, idx| {
    try_rewrite_single_loose_record_to_struct_record(arg, expected, file_ns, def_name, &fn_info.name, idx, check_warnings)
  })
}

/// Rewrite untyped tuple literal arguments to enum tuples when enum-typed.
pub(crate) fn try_rewrite_tuple_args_to_enum_tuples(
  fn_info: &CalcitFn,
  processed_args: &CalcitList,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Option<CalcitList> {
  rewrite_args_by_expected_type(&fn_info.arg_types, processed_args, |arg, expected, idx| {
    try_rewrite_single_tuple_to_enum_tuple(arg, expected, file_ns, def_name, &fn_info.name, idx, check_warnings)
  })
}

// ---------------------------------------------------------------------------
// Single-argument rewrite implementations
// ---------------------------------------------------------------------------

/// Try to rewrite a single hashmap literal to a record literal if the expected type is a struct.
fn try_rewrite_single_map_to_record(
  arg: &Calcit,
  expected_type: &Arc<CalcitTypeAnnotation>,
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
            "[Warn] map-to-record rewrite skipped for `{fn_name}` arg {}: key `:{key}` is not a field of struct, at {file_ns}/{def_name}",
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
          "[Warn] map-to-record rewrite skipped for `{fn_name}` arg {}: non-tag key `{}` found, at {file_ns}/{def_name}",
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

  // Build the rewritten record literal with ALL struct fields in order.
  // Fields not provided in the map get nil.
  let mut record_items: Vec<Calcit> = Vec::with_capacity(struct_def.fields.len() * 2 + 2);
  record_items.push(Calcit::Proc(CalcitProc::NativeRecord));
  record_items.push(struct_ref_node);
  for field in struct_def.fields.iter() {
    record_items.push(Calcit::Tag(field.to_owned()));
    if let Some(value) = provided_fields.get(field) {
      record_items.push((*value).to_owned());
    } else {
      record_items.push(Calcit::Nil);
    }
  }

  Some(Calcit::from(record_items))
}

/// Try to rewrite a single loose record literal (`?{} :field val ...`) to a struct record literal.
fn try_rewrite_single_loose_record_to_struct_record(
  arg: &Calcit,
  expected_type: &Arc<CalcitTypeAnnotation>,
  file_ns: &str,
  def_name: &str,
  fn_name: &str,
  arg_idx: usize,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Option<Calcit> {
  // Only handle loose record literals (lists starting with Proc(NativeLooseRecord))
  let Calcit::List(arg_list) = arg else { return None };
  let Some(Calcit::Proc(CalcitProc::NativeLooseRecord)) = arg_list.first() else {
    return None;
  };

  // Resolve the expected type to a struct definition + optional ns/def path
  let (struct_def, ns_def_path) = expected_type.resolve_to_struct_with_ref()?;

  // Validate: the loose record literal has flat key-value pairs after the NativeLooseRecord head
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
            "[Warn] loose-record-to-struct rewrite skipped for `{fn_name}` arg {}: key `:{key}` is not a field of struct `{}`, at {file_ns}/{def_name}",
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
          "[Warn] loose-record-to-struct rewrite skipped for `{fn_name}` arg {}: non-tag key `{}` found, at {file_ns}/{def_name}",
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

  // Build the rewritten record literal with ALL struct fields in order.
  let mut record_items: Vec<Calcit> = Vec::with_capacity(struct_def.fields.len() * 2 + 2);
  record_items.push(Calcit::Proc(CalcitProc::NativeRecord));
  record_items.push(struct_ref_node);
  for field in struct_def.fields.iter() {
    record_items.push(Calcit::Tag(field.to_owned()));
    if let Some(value) = provided_fields.get(field) {
      record_items.push((*value).to_owned());
    } else {
      record_items.push(Calcit::Nil);
    }
  }

  Some(Calcit::from(record_items))
}

/// Try to rewrite a single untyped tuple literal (`:: :tag payload...`) to a typed enum tuple
/// (`%:: EnumDef :tag payload...`) if the expected type is an enum.
fn try_rewrite_single_tuple_to_enum_tuple(
  arg: &Calcit,
  expected_type: &Arc<CalcitTypeAnnotation>,
  file_ns: &str,
  def_name: &str,
  fn_name: &str,
  arg_idx: usize,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Option<Calcit> {
  // Only handle untyped tuple literals (lists starting with Proc(NativeTuple))
  let Calcit::List(arg_list) = arg else { return None };
  let Some(Calcit::Proc(CalcitProc::NativeTuple)) = arg_list.first() else {
    return None;
  };

  // Resolve the expected type to an enum definition + optional ns/def path
  let (enum_def, ns_def_path) = expected_type.resolve_to_enum_with_ref()?;

  // Validate: the tuple literal needs at least a tag
  if arg_list.len() < 2 {
    gen_check_warning(
      format!(
        "[Warn] tuple-to-enum rewrite skipped for `{fn_name}` arg {}: tuple literal has no tag, at {file_ns}/{def_name}",
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

  // Build rewritten: [NativeEnumTupleNew, enum_ref, tag, ...payloads]
  let mut items: Vec<Calcit> = Vec::with_capacity(arg_list.len() + 1);
  items.push(Calcit::Proc(CalcitProc::NativeEnumTupleNew));
  items.push(enum_ref_node);
  // Copy tag and payloads from the original tuple literal (skip NativeTuple head)
  for item in arg_list.iter().skip(1) {
    items.push(item.to_owned());
  }

  Some(Calcit::from(items))
}

// ---------------------------------------------------------------------------
// Shared reference node builders
// ---------------------------------------------------------------------------

/// Build a Calcit node referencing a struct definition, using Import when ns/def is known.
fn build_struct_ref_node(
  struct_def: &CalcitStruct,
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
    Calcit::Struct(struct_def.clone())
  }
}

/// Build a Calcit node referencing an enum definition, using Import when ns/def is known.
fn build_enum_ref_node(enum_def: CalcitEnum, ns_def_path: Option<(Arc<str>, Arc<str>)>, file_ns: &str, def_name: &str) -> Calcit {
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
    Calcit::Enum(enum_def)
  }
}
