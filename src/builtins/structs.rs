use std::collections::HashMap;
use std::ops::Rem;
use std::sync::Arc;

use cirru_edn::EdnTag;

use crate::builtins::meta::type_of;
use crate::calcit::CORE_NS;
use crate::calcit::type_annotation::{collect_runtime_type_bindings, validate_runtime_generic_where_bounds};
use crate::calcit::{
  Calcit, CalcitEnumDef, CalcitErr, CalcitErrKind, CalcitImpl, CalcitImport, CalcitList, CalcitProc, CalcitStructDef,
  CalcitStructValue, CalcitSyntax, CalcitTypeAnnotation, brief_type_of_value, format_proc_examples_hint, value_matches_type_annotation,
};

fn mark_fn_used_in_impl(value: &Calcit) -> Calcit {
  match value {
    Calcit::Fn { id, info } => {
      let mut updated = info.as_ref().to_owned();
      updated.usage.used_in_impl = true;
      Calcit::Fn {
        id: id.to_owned(),
        info: Arc::new(updated),
      }
    }
    _ => value.to_owned(),
  }
}

fn callable_type(value: &Calcit) -> Option<CalcitTypeAnnotation> {
  match value {
    Calcit::Fn { info, .. } => Some(CalcitTypeAnnotation::from_calcit_fn(info)),
    Calcit::Proc(proc) => proc
      .get_type_signature()
      .map(|signature| CalcitTypeAnnotation::from_function_parts(signature.arg_types.clone(), signature.return_type.clone())),
    _ => None,
  }
}

fn validate_trait_impl_entries(trait_def: &crate::calcit::CalcitTrait, entries: &[(EdnTag, Calcit)]) -> Result<(), CalcitErr> {
  let missing = trait_def
    .methods
    .iter()
    .filter(|method| !entries.iter().any(|(name, _)| name == *method))
    .map(ToString::to_string)
    .collect::<Vec<_>>();
  let unexpected = entries
    .iter()
    .filter(|(name, _)| !trait_def.has_method(name.ref_str()))
    .map(|(name, _)| name.to_string())
    .collect::<Vec<_>>();

  if !missing.is_empty() || !unexpected.is_empty() {
    let mut details = vec![];
    if !missing.is_empty() {
      details.push(format!("missing methods: {}", missing.join(" ")));
    }
    if !unexpected.is_empty() {
      details.push(format!("methods not declared by the trait: {}", unexpected.join(" ")));
    }
    return Err(CalcitErr::use_str(
      CalcitErrKind::Type,
      format!("&impl::new does not conform to trait {}: {}", trait_def.name, details.join("; ")),
    ));
  }

  for (name, value) in entries {
    let Some(method_idx) = trait_def.method_index(name.ref_str()) else {
      continue;
    };
    let expected = trait_def
      .method_types
      .get(method_idx)
      .expect("trait method type must align with its name");
    let Some(actual) = callable_type(value) else {
      return Err(CalcitErr::use_str(
        CalcitErrKind::Type,
        format!(
          "&impl::new expects trait method .{} to be a function, but received: {value}",
          name.ref_str()
        ),
      ));
    };

    if matches!(expected.as_ref(), CalcitTypeAnnotation::DynFn) {
      continue;
    }
    // A builtin without registered type metadata is still callable, but there is
    // no useful signature evidence to compare. User functions always preserve
    // their arity and available hints through `from_calcit_fn` above.
    if matches!(value, Calcit::Proc(proc) if proc.get_type_signature().is_none()) {
      continue;
    }
    if !actual.matches_annotation(expected.as_ref()) {
      return Err(CalcitErr::use_str(
        CalcitErrKind::Type,
        format!(
          "&impl::new method .{} does not match trait {} signature: expected {}, got {}",
          name.ref_str(),
          trait_def.name,
          expected.to_brief_string(),
          actual.to_brief_string()
        ),
      ));
    }
  }

  Ok(())
}

fn parse_type_var_form(form: &Calcit) -> Option<Arc<str>> {
  let Calcit::List(list) = form else {
    return None;
  };

  let head = list.first()?;
  let is_quote_head =
    matches!(head, Calcit::Syntax(CalcitSyntax::Quote, _)) || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "quote");

  if !is_quote_head {
    return None;
  }

  match list.get(1) {
    Some(Calcit::Symbol { sym, .. }) => Some(sym.to_owned()),
    _ => None,
  }
}

fn parse_generics_list(form: &Calcit) -> Option<Vec<Arc<str>>> {
  let Calcit::List(items) = form else {
    return None;
  };

  let start = if matches!(items.first(), Some(Calcit::Proc(CalcitProc::List))) {
    1
  } else {
    0
  };

  let mut vars = Vec::with_capacity(items.len());
  for item in items.iter().skip(start) {
    if let Some(name) = parse_type_var_form(item) {
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

fn is_list_literal_head(form: &Calcit) -> bool {
  match form {
    Calcit::Symbol { sym, .. } => sym.as_ref() == "[]",
    Calcit::Proc(CalcitProc::List) => true,
    Calcit::Import(CalcitImport { ns, def, .. }) => ns.as_ref() == CORE_NS && def.as_ref() == "[]",
    Calcit::Macro { info, .. } => info.def_ns.as_ref() == CORE_NS && info.name.as_ref() == "[]",
    _ => false,
  }
}

fn is_where_map_head(form: &Calcit) -> bool {
  matches!(form, Calcit::Symbol { sym, .. } if sym.as_ref() == "{}")
    || matches!(form, Calcit::Proc(CalcitProc::NativeMap))
    || matches!(form, Calcit::Import(CalcitImport { ns, def, .. }) if ns.as_ref() == CORE_NS && def.as_ref() == "{}")
    || matches!(form, Calcit::Macro { info, .. } if info.def_ns.as_ref() == CORE_NS && info.name.as_ref() == "{}")
}

fn normalize_where_bounds_form(form: &Calcit) -> Option<Calcit> {
  match form {
    Calcit::Map(_) => Some(form.to_owned()),
    Calcit::List(items) => {
      if items.first().is_some_and(is_where_map_head) {
        Some(form.to_owned())
      } else if items.first().is_some_and(is_list_literal_head) && items.get(1).is_some_and(is_where_map_head) {
        Some(Calcit::List(Arc::new(CalcitList::Vector(items.iter().skip(1).cloned().collect()))))
      } else {
        None
      }
    }
    _ => None,
  }
}

fn parse_where_bounds(form: &Calcit, generics: &[Arc<str>]) -> Option<Vec<crate::calcit::CalcitGenericBound>> {
  let normalized = normalize_where_bounds_form(form)?;
  Some(CalcitTypeAnnotation::parse_where_bounds_form(&normalized, generics, true))
}

pub fn new_impl(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&impl::new expected arguments, but received none:", xs);
  }
  let (name_id, origin) = match &xs[0] {
    Calcit::Trait(trait_def) => (trait_def.name.to_owned(), Some(Arc::new(trait_def.to_owned()))),
    Calcit::Symbol { sym, .. } => (EdnTag(sym.to_owned()), None),
    Calcit::Tag(k) => (k.to_owned(), None),
    Calcit::Str(s) => (EdnTag(s.to_owned()), None),
    a => {
      let msg = format!(
        "&impl::new requires a trait or name (symbol/tag/string), but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeImplNew).unwrap_or_default();
      return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
    }
  };

  let mut entries: Vec<(EdnTag, Calcit)> = Vec::with_capacity(xs.len().saturating_sub(1));
  let items: Vec<Calcit> = if let [_, Calcit::Impl(source)] = xs {
    source
      .fields
      .iter()
      .zip(source.values.iter())
      .map(|(field, value)| Calcit::from(vec![Calcit::Tag(field.clone()), value.clone()]))
      .collect()
  } else {
    xs.iter().skip(1).cloned().collect()
  };
  for item in &items {
    let (name, value) = match item {
      Calcit::List(pair) => match (pair.first(), pair.get(1), pair.get(2)) {
        (Some(name), Some(value), None) => (name, value),
        _ => {
          let msg = format!("&impl::new expects (field value) pairs, but received: {item}");
          let hint = format_proc_examples_hint(&CalcitProc::NativeImplNew).unwrap_or_default();
          return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
        }
      },
      Calcit::Enum(enum_value) => match (enum_value.extra.first(), enum_value.extra.get(1)) {
        (Some(value), None) => (enum_value.tag.as_ref(), value),
        _ => {
          let msg = format!("&impl::new expects (field value) pairs, but received: {item}");
          let hint = format_proc_examples_hint(&CalcitProc::NativeImplNew).unwrap_or_default();
          return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
        }
      },
      other => {
        let msg = format!("&impl::new expects pair lists or tuples, but received: {other}");
        let hint = format_proc_examples_hint(&CalcitProc::NativeImplNew).unwrap_or_default();
        return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
      }
    };

    let field_name = match name {
      Calcit::Symbol { sym, .. } | Calcit::Str(sym) => EdnTag(sym.to_owned()),
      Calcit::Tag(tag) => tag.to_owned(),
      Calcit::Method(sym, _) => EdnTag(sym.to_owned()),
      other => {
        let msg = format!("&impl::new field expects tag/symbol/string/.method, but received: {other}");
        let hint = format_proc_examples_hint(&CalcitProc::NativeImplNew).unwrap_or_default();
        return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
      }
    };
    let value = mark_fn_used_in_impl(value);
    entries.push((field_name, value));
  }

  entries.sort_by(|a, b| a.0.ref_str().cmp(b.0.ref_str()));
  for idx in 1..entries.len() {
    if entries[idx - 1].0 == entries[idx].0 {
      return CalcitErr::err_str(
        CalcitErrKind::Unexpected,
        format!("&impl::new duplicated field: {}", entries[idx].0),
      );
    }
  }

  if let Some(trait_def) = origin.as_ref() {
    validate_trait_impl_entries(trait_def, &entries)?;
  }

  let fields: Vec<EdnTag> = entries.iter().map(|(tag, _)| tag.to_owned()).collect();
  let values: Vec<Calcit> = entries.into_iter().map(|(_, v)| v).collect();

  Ok(Calcit::Impl(CalcitImpl {
    name: name_id,
    origin,
    fields: Arc::new(fields),
    values: Arc::new(values),
  }))
}

pub fn new_struct(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    let hint = format_proc_examples_hint(&CalcitProc::NativeStructNew).unwrap_or_default();
    return CalcitErr::err_nodes_with_hint(
      CalcitErrKind::Arity,
      "&struct-def:new expects a name and field definitions, but received none:",
      xs,
      hint,
    );
  }

  let name_id: EdnTag = match &xs[0] {
    Calcit::Symbol { sym, .. } => EdnTag(sym.to_owned()),
    Calcit::Tag(k) => k.to_owned(),
    a => {
      let msg = format!(
        "&struct-def:new expects a name (symbol or tag), but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeStructNew).unwrap_or_default();
      return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
    }
  };

  let mut generics: Vec<Arc<str>> = vec![];
  let mut start_idx = 1;
  if let Some(generics_form) = xs.get(1).and_then(parse_generics_list) {
    generics = generics_form;
    start_idx = 2;
  }
  let mut where_bounds = vec![];
  if let Some(form) = xs.get(start_idx).and_then(|form| parse_where_bounds(form, generics.as_slice())) {
    where_bounds = form;
    start_idx += 1;
  }

  let mut fields: Vec<(EdnTag, Arc<CalcitTypeAnnotation>)> = vec![];
  for item in xs.iter().skip(start_idx) {
    match item {
      Calcit::List(xs) => match (xs.first(), xs.get(1), xs.get(2)) {
        (Some(name), Some(type_expr), None) => {
          let field_name = match name {
            Calcit::Symbol { sym, .. } | Calcit::Str(sym) => EdnTag(sym.to_owned()),
            Calcit::Tag(tag) => tag.to_owned(),
            other => {
              let msg = format!("&struct-def:new field expects a tag/symbol, but received: {other}");
              let hint = format_proc_examples_hint(&CalcitProc::NativeStructNew).unwrap_or_default();
              return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
            }
          };
          let field_type = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(type_expr, generics.as_slice());
          if let Err(e) = field_type.validate_applied_type_args() {
            let hint = format_proc_examples_hint(&CalcitProc::NativeStructNew).unwrap_or_default();
            return CalcitErr::err_str_with_hint(
              CalcitErrKind::Type,
              format!("&struct-def:new field `{field_name}` has invalid type annotation: {e}"),
              hint,
            );
          }
          fields.push((field_name, field_type));
        }
        (Some(_), None, _) => {
          let hint = format_proc_examples_hint(&CalcitProc::NativeStructNew).unwrap_or_default();
          return CalcitErr::err_str_with_hint(
            CalcitErrKind::Arity,
            "&struct-def:new field expects a pair (field type), but received only a field name",
            hint,
          );
        }
        _ => {
          let msg = format!("&struct-def:new field expects a pair list, but received: {item}");
          let hint = format_proc_examples_hint(&CalcitProc::NativeStructNew).unwrap_or_default();
          return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
        }
      },
      other => {
        let msg = format!("&struct-def:new expects field entries as lists, but received: {other}");
        let hint = format_proc_examples_hint(&CalcitProc::NativeStructNew).unwrap_or_default();
        return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
      }
    }
  }

  fields.sort_by(|a, b| a.0.ref_str().cmp(b.0.ref_str()));
  for idx in 1..fields.len() {
    if fields[idx - 1].0 == fields[idx].0 {
      return CalcitErr::err_str(
        CalcitErrKind::Unexpected,
        format!("&struct-def:new duplicated field: {}", fields[idx].0),
      );
    }
  }

  generics.sort();
  generics.dedup();

  let field_names: Vec<EdnTag> = fields.iter().map(|(name, _)| name.to_owned()).collect();
  let field_types: Vec<Arc<CalcitTypeAnnotation>> = fields.iter().map(|(_, t)| t.to_owned()).collect();

  Ok(Calcit::StructDef(CalcitStructDef {
    name: name_id,
    fields: Arc::new(field_names),
    field_types: Arc::new(field_types),
    generics: Arc::new(generics),
    where_bounds: Arc::new(where_bounds),
    impls: vec![],
  }))
}

pub fn new_enum(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    let hint = format_proc_examples_hint(&CalcitProc::NativeEnumNew).unwrap_or_default();
    return CalcitErr::err_nodes_with_hint(
      CalcitErrKind::Arity,
      "&enum-def:new expects a name and variants, but received none:",
      xs,
      hint,
    );
  }

  let name_id: EdnTag = match &xs[0] {
    Calcit::Symbol { sym, .. } => EdnTag(sym.to_owned()),
    Calcit::Tag(k) => k.to_owned(),
    a => {
      let msg = format!(
        "&enum-def:new expects a name (symbol or tag), but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeEnumNew).unwrap_or_default();
      return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
    }
  };

  let mut generics: Vec<Arc<str>> = vec![];
  let mut start_idx = 1;
  if let Some(generics_form) = xs.get(1).and_then(parse_generics_list) {
    generics = generics_form;
    start_idx = 2;
  }
  let mut where_bounds = vec![];
  if let Some(form) = xs.get(start_idx).and_then(|form| parse_where_bounds(form, generics.as_slice())) {
    where_bounds = form;
    start_idx += 1;
  }

  let mut variants: Vec<(EdnTag, Calcit)> = vec![];
  for item in xs.iter().skip(start_idx) {
    match item {
      Calcit::List(xs) => {
        let tag = match xs.first() {
          Some(Calcit::Symbol { sym, .. }) => EdnTag(sym.to_owned()),
          Some(Calcit::Tag(k)) => k.to_owned(),
          Some(other) => {
            let msg = format!("&enum-def:new variant expects a tag, but received: {other}");
            let hint = format_proc_examples_hint(&CalcitProc::NativeEnumNew).unwrap_or_default();
            return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
          }
          None => {
            let hint = format_proc_examples_hint(&CalcitProc::NativeEnumNew).unwrap_or_default();
            return CalcitErr::err_str_with_hint(
              CalcitErrKind::Arity,
              "&enum-def:new variant expects a tag and payload types, but received an empty list",
              hint,
            );
          }
        };

        let payloads = xs.drop_left();
        let payload_list = Calcit::List(Arc::new(CalcitList::Vector(payloads.to_vec())));
        variants.push((tag, payload_list));
      }
      other => {
        let msg = format!("&enum-def:new expects variants as lists, but received: {other}");
        let hint = format_proc_examples_hint(&CalcitProc::NativeEnumNew).unwrap_or_default();
        return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
      }
    }
  }

  variants.sort_by(|a, b| a.0.ref_str().cmp(b.0.ref_str()));
  for idx in 1..variants.len() {
    if variants[idx - 1].0 == variants[idx].0 {
      return CalcitErr::err_str(
        CalcitErrKind::Unexpected,
        format!("&enum-def:new duplicated variant: {}", variants[idx].0),
      );
    }
  }

  let fields: Vec<EdnTag> = variants.iter().map(|(tag, _)| tag.to_owned()).collect();
  let values: Vec<Calcit> = variants.iter().map(|(_, value)| value.to_owned()).collect();

  let mut struct_ref = CalcitStructDef::from_fields(name_id, fields);
  generics.sort();
  generics.dedup();
  struct_ref.generics = Arc::new(generics);
  struct_ref.where_bounds = Arc::new(where_bounds);
  struct_ref.impls = vec![Arc::new(enum_prototype_marker())];

  let struct_value = CalcitStructValue {
    struct_ref: Arc::new(struct_ref),
    values: Arc::new(values),
  };

  match CalcitEnumDef::from_record(struct_value) {
    Ok(enum_def) => Ok(Calcit::EnumDef(enum_def)),
    Err(msg) => CalcitErr::err_str(CalcitErrKind::Type, format!("&enum-def:new failed to build enum: {msg}")),
  }
}

fn enum_prototype_marker() -> CalcitImpl {
  CalcitImpl {
    name: EdnTag::new("enum-prototype"),
    origin: None,
    fields: Arc::new(vec![]),
    values: Arc::new(vec![]),
  }
}

/// Partial struct constructor — missing fields default to nil.
/// Proto must be a `Calcit::StructDef` from `defstruct`.
pub fn call_struct_partial(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let args_size = xs.len();
  if args_size < 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&%{}? expected at least 1 argument, but received:", xs);
  }
  if (args_size - 1).rem(2) != 0 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&%{}? expected pairs after prototype, but received:", xs);
  }
  let (base_struct, mut base_values): (Arc<CalcitStructDef>, Vec<Calcit>) = match &xs[0] {
    Calcit::StructDef(s) => {
      let vals = vec![Calcit::Nil; s.fields.len()];
      (Arc::new(s.to_owned()), vals)
    }
    a => {
      return CalcitErr::err_str(
        CalcitErrKind::Type,
        format!(
          "&%{{}}? requires a struct as prototype, but received: {}",
          type_of(std::slice::from_ref(a))?.lisp_str()
        ),
      );
    }
  };
  let size = (args_size - 1) / 2;
  let mut seen_positions: Vec<bool> = vec![false; base_struct.fields.len()];
  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();
  for idx in 0..size {
    let k_idx = idx * 2 + 1;
    let v_idx = k_idx + 1;
    let field_name: &str = match &xs[k_idx] {
      Calcit::Tag(s) => s.ref_str(),
      Calcit::Symbol { sym: s, .. } | Calcit::Str(s) => s,
      a => {
        return CalcitErr::err_str(
          CalcitErrKind::Type,
          format!(
            "&%{{}}? requires field in string/tag, but received: {}",
            type_of(std::slice::from_ref(a))?.lisp_str()
          ),
        );
      }
    };
    match base_struct.fields.iter().position(|f| f.ref_str() == field_name) {
      Some(pos) => {
        if seen_positions[pos] {
          return CalcitErr::err_str(CalcitErrKind::Type, format!("&%{{}}? duplicate field: :{field_name}"));
        }
        seen_positions[pos] = true;
        // Validate field value type against struct field_types
        if let Some(expected_type) = base_struct.field_types.get(pos) {
          if !matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic)
            && !value_matches_type_annotation(&xs[v_idx], expected_type)
          {
            return CalcitErr::err_str(
              CalcitErrKind::Type,
              format!(
                "&%{{}}? field `{}` expects type `{}`, but received `{}` ({})",
                field_name,
                expected_type.to_brief_string(),
                brief_type_of_value(&xs[v_idx]),
                xs[v_idx].lisp_str()
              ),
            );
          }
          collect_runtime_type_bindings(&xs[v_idx], expected_type.as_ref(), &mut bindings);
        }
        xs[v_idx].clone_into(&mut base_values[pos]);
      }
      None => {
        return CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("&%{{}}? unexpected field `{field_name}` for struct: {:?}", base_struct.fields),
        );
      }
    }
  }
  if let Err(msg) = validate_runtime_generic_where_bounds(&bindings, base_struct.where_bounds.as_ref()) {
    return CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&%{{}}? failed generic where-bound validation for `{}`: {msg}", base_struct.name),
    );
  }
  Ok(Calcit::Struct(CalcitStructValue {
    struct_ref: base_struct,
    values: Arc::new(base_values),
  }))
}

/// Create a loose struct from key-value pairs: `?{} :field1 val1 :field2 val2`
/// Fields are sorted alphabetically, mirroring named struct behaviour.
pub fn call_loose_struct(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len().rem(2) != 0 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "?{} expected pairs of :field value, but received:", xs);
  }
  let size = xs.len() / 2;
  // Collect (field, value) pairs, validate all keys are tags
  let mut pairs: Vec<(EdnTag, Calcit)> = Vec::with_capacity(size);
  for chunk in xs.chunks(2) {
    match &chunk[0] {
      Calcit::Tag(tag) => {
        pairs.push((tag.to_owned(), chunk[1].to_owned()));
      }
      other => {
        return CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("?{{}} expected tag as field name, but received: {}", other.lisp_str()),
        );
      }
    }
  }
  // Sort by field name (struct fields are always sorted)
  pairs.sort_by(|a, b| a.0.ref_str().cmp(b.0.ref_str()));
  // Check for duplicate fields
  for i in 1..pairs.len() {
    if pairs[i].0 == pairs[i - 1].0 {
      return CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("?{{}} received duplicate field: :{}", pairs[i].0.ref_str()),
      );
    }
  }
  let fields: Vec<EdnTag> = pairs.iter().map(|(f, _)| f.to_owned()).collect();
  let values: Vec<Calcit> = pairs.into_iter().map(|(_, v)| v).collect();
  Ok(Calcit::Struct(CalcitStructValue::from_anonymous_pairs(fields, values)))
}

/// Direct indexed access to a struct field: `&struct:nth struct_value index`
/// This is the optimized path emitted by the preprocessor when the field index is known at compile time.
pub fn struct_nth(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  // Accept 2 or 3 args: (struct, idx) or (struct, idx, :field-tag)
  // The 3rd arg (field tag) is only used by JS codegen; Rust runtime ignores it.
  if xs.len() < 2 || xs.len() > 3 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:nth expected 2-3 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Struct(CalcitStructValue { values, struct_ref }), Calcit::Number(n)) => {
      let idx = *n as usize;
      if idx < values.len() {
        Ok(values[idx].to_owned())
      } else {
        CalcitErr::err_str(
          CalcitErrKind::Arity,
          format!(
            "&struct:nth index {} out of range for struct `{}` with {} fields",
            idx,
            struct_ref.name,
            values.len()
          ),
        )
      }
    }
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!(
        "&struct:nth expected (struct, number), but received: {} {}",
        a.lisp_str(),
        b.lisp_str()
      ),
    ),
  }
}

/// Get the field tag (name) at a given index: `&struct:field-tag struct_value index`
pub fn struct_field_tag(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:field-tag expected 2 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Struct(CalcitStructValue { struct_ref, .. }), Calcit::Number(n)) => {
      let idx = *n as usize;
      if idx < struct_ref.fields.len() {
        Ok(Calcit::Tag(struct_ref.fields[idx].to_owned()))
      } else {
        CalcitErr::err_str(
          CalcitErrKind::Arity,
          format!(
            "&struct:field-tag index {} out of range for struct `{}` with {} fields",
            idx,
            struct_ref.name,
            struct_ref.fields.len()
          ),
        )
      }
    }
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!(
        "&struct:field-tag expected (struct, number), but received: {} {}",
        a.lisp_str(),
        b.lisp_str()
      ),
    ),
  }
}

/// Direct indexed assoc on a struct field: `&struct:assoc-at struct_value index :field value`
/// This is the optimized path emitted by the preprocessor when the field index is known at compile time.
pub fn struct_assoc_at(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  // 4 args: (struct, idx, :field-tag, value)
  // Keep the field tag as a runtime consistency check so a stale index cannot
  // silently update an adjacent field after schema drift.
  if xs.len() != 4 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:assoc-at expected 4 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1], &xs[2]) {
    (Calcit::Struct(CalcitStructValue { struct_ref, values }), Calcit::Number(n), Calcit::Tag(field_tag)) => {
      let idx = checked_struct_index(*n, "&struct:assoc-at")?;
      if idx < values.len() {
        let Some(expected_tag) = struct_ref.fields.get(idx) else {
          return CalcitErr::err_str(
            CalcitErrKind::Arity,
            format!(
              "&struct:assoc-at struct `{}` is missing field metadata at index {idx}",
              struct_ref.name
            ),
          );
        };
        if expected_tag != field_tag {
          return CalcitErr::err_str(
            CalcitErrKind::Type,
            format!("&struct:assoc-at index {idx} expects field `:{expected_tag}`, but received `:{field_tag}`"),
          );
        }
        let mut new_values = (**values).to_owned();
        xs[3].clone_into(&mut new_values[idx]);
        Ok(Calcit::Struct(CalcitStructValue {
          struct_ref: struct_ref.to_owned(),
          values: Arc::new(new_values),
        }))
      } else {
        CalcitErr::err_str(
          CalcitErrKind::Arity,
          format!(
            "&struct:assoc-at index {} out of range for struct `{}` with {} fields",
            idx,
            struct_ref.name,
            values.len()
          ),
        )
      }
    }
    (a, b, field) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!(
        "&struct:assoc-at expected (struct, number, tag), but received: {} {} {}",
        a.lisp_str(),
        b.lisp_str(),
        field.lisp_str()
      ),
    ),
  }
}

/// Optimized `&struct:with` — field indices pre-resolved at compile time.
/// Args: (struct, idx1, :tag1, val1, idx2, :tag2, val2, ...)
/// Tags are carried for JS codegen; Rust runtime uses indices directly.
pub fn struct_with_at(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() || !(xs.len() - 1).is_multiple_of(3) {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&struct:with-at expected (struct, idx, tag, val, ...) triples, but received:",
      xs,
    );
  }
  match &xs[0] {
    Calcit::Struct(CalcitStructValue { struct_ref, values }) => {
      let mut new_values = (**values).to_owned();
      let triple_count = (xs.len() - 1) / 3;
      for i in 0..triple_count {
        let base = 1 + i * 3;
        match (&xs[base], &xs[base + 1]) {
          (Calcit::Number(n), Calcit::Tag(field_tag)) => {
            let idx = checked_struct_index(*n, "&struct:with-at")?;
            if idx < new_values.len() {
              let Some(expected_tag) = struct_ref.fields.get(idx) else {
                return CalcitErr::err_str(
                  CalcitErrKind::Arity,
                  format!(
                    "&struct:with-at struct `{}` is missing field metadata at index {idx}",
                    struct_ref.name
                  ),
                );
              };
              if expected_tag != field_tag {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!("&struct:with-at index {idx} expects field `:{expected_tag}`, but received `:{field_tag}`"),
                );
              }
              xs[base + 2].clone_into(&mut new_values[idx]);
            } else {
              return CalcitErr::err_str(
                CalcitErrKind::Arity,
                format!(
                  "&struct:with-at index {} out of range for struct `{}` with {} fields",
                  idx,
                  struct_ref.name,
                  new_values.len()
                ),
              );
            }
          }
          (index, field) => {
            return CalcitErr::err_str(
              CalcitErrKind::Type,
              format!(
                "&struct:with-at expected number index and tag, but received: {} {}",
                index.lisp_str(),
                field.lisp_str()
              ),
            );
          }
        }
      }
      Ok(Calcit::Struct(CalcitStructValue {
        struct_ref: struct_ref.to_owned(),
        values: Arc::new(new_values),
      }))
    }
    a => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&struct:with-at expected a struct value, but received: {}", a.lisp_str()),
    ),
  }
}

fn checked_struct_index(value: f64, operation: &str) -> Result<usize, CalcitErr> {
  if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
    return Err(CalcitErr::use_str(
      CalcitErrKind::Type,
      format!("{operation} expected a non-negative integer index, but received: {value}"),
    ));
  }
  Ok(value as usize)
}

pub fn call_struct(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let args_size = xs.len();
  if args_size < 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&%{{}} expected at least 2 arguments, but received:", xs);
  }
  match &xs[0] {
    Calcit::StructDef(struct_def) => {
      let struct_value = CalcitStructValue {
        struct_ref: Arc::new(struct_def.to_owned()),
        values: Arc::new(vec![Calcit::Nil; struct_def.fields.len()]),
      };
      call_struct_with_prototype(&struct_value, xs)
    }
    Calcit::Struct(_) => CalcitErr::err_str(
      CalcitErrKind::Type,
      "&%{} requires a struct (from defstruct) as prototype, not a struct instance; use defstruct to define the type",
    ),
    a => {
      let msg = format!(
        "&%{{}} requires a struct as prototype, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecord).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

fn call_struct_with_prototype(struct_value: &CalcitStructValue, xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let args_size = xs.len();
  let CalcitStructValue { struct_ref, values: v0 } = struct_value;
  if (args_size - 1).rem(2) != 0 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&%{{}} expected pairs, but received:", xs);
  }
  let size = (args_size - 1) / 2;
  if size != struct_ref.fields.len() {
    return CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!(
        "&%{{}} unexpected number of fields. Expected {}, but received {}",
        struct_ref.fields.len(),
        size
      ),
    );
  }
  let mut values: Vec<Calcit> = (**v0).to_owned();
  let mut seen_positions: Vec<bool> = vec![false; struct_ref.fields.len()];
  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();

  for idx in 0..size {
    let k_idx = idx * 2 + 1;
    let v_idx = k_idx + 1;
    match &xs[k_idx] {
      Calcit::Tag(s) => match struct_value.index_of(s.ref_str()) {
        Some(pos) => {
          if seen_positions[pos] {
            return CalcitErr::err_str(CalcitErrKind::Type, format!("&%{{{{}}}} duplicate field: :{}", s.ref_str()));
          }
          seen_positions[pos] = true;
          // Validate field value type against struct field_types
          if let Some(expected_type) = struct_ref.field_types.get(pos) {
            if !matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic)
              && !value_matches_type_annotation(&xs[v_idx], expected_type)
            {
              return CalcitErr::err_str(
                CalcitErrKind::Type,
                format!(
                  "&%{{}} field `{}` expects type `{}`, but received `{}` ({})",
                  s.ref_str(),
                  expected_type.to_brief_string(),
                  brief_type_of_value(&xs[v_idx]),
                  xs[v_idx].lisp_str()
                ),
              );
            }
            collect_runtime_type_bindings(&xs[v_idx], expected_type.as_ref(), &mut bindings);
          }
          xs[v_idx].clone_into(&mut values[pos]);
        }
        None => {
          return CalcitErr::err_str(
            CalcitErrKind::Type,
            format!("&%{{}} unexpected field `{s}` for struct: {:?}", struct_ref.fields),
          );
        }
      },
      Calcit::Symbol { sym: s, .. } | Calcit::Str(s) => match struct_value.index_of(s) {
        Some(pos) => {
          // Validate field value type against struct field_types
          if let Some(expected_type) = struct_ref.field_types.get(pos) {
            if !matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic)
              && !value_matches_type_annotation(&xs[v_idx], expected_type)
            {
              return CalcitErr::err_str(
                CalcitErrKind::Type,
                format!(
                  "&%{{}} field `{}` expects type `{}`, but received `{}` ({})",
                  s,
                  expected_type.to_brief_string(),
                  brief_type_of_value(&xs[v_idx]),
                  xs[v_idx].lisp_str()
                ),
              );
            }
            collect_runtime_type_bindings(&xs[v_idx], expected_type.as_ref(), &mut bindings);
          }
          xs[v_idx].clone_into(&mut values[pos]);
        }
        None => {
          return CalcitErr::err_str(
            CalcitErrKind::Type,
            format!("&%{{}} unexpected field `{s}` for struct: {:?}", struct_ref.fields),
          );
        }
      },
      a => {
        let msg = format!(
          "&%{{}} requires field in string/tag, but received: {}",
          type_of(std::slice::from_ref(a))?.lisp_str()
        );
        let hint = format_proc_examples_hint(&CalcitProc::NativeRecord).unwrap_or_default();
        return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
      }
    }
  }

  if let Err(msg) = validate_runtime_generic_where_bounds(&bindings, struct_ref.where_bounds.as_ref()) {
    return CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&%{{}} failed generic where-bound validation for `{}`: {msg}", struct_ref.name),
    );
  }

  Ok(Calcit::Struct(CalcitStructValue {
    struct_ref: struct_ref.to_owned(),
    values: Arc::new(values),
  }))
}

/// takes a struct value and pairs of key value (flattened), and updates the struct. raise error if key not existed in the struct
pub fn struct_with(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let args_size = xs.len();
  if args_size < 3 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&struct:with expected at least 3 arguments, but received:",
      xs,
    );
  }
  match &xs[0] {
    Calcit::Struct(struct_value @ CalcitStructValue { struct_ref, values: v0 }) => {
      if (args_size - 1).rem(2) == 0 {
        let size = (args_size - 1) / 2;
        let mut values: Vec<Calcit> = (**v0).to_owned();

        for idx in 0..size {
          let k_idx = idx * 2 + 1;
          let v_idx = k_idx + 1;
          match &xs[k_idx] {
            Calcit::Tag(s) => match struct_value.index_of(s.ref_str()) {
              Some(pos) => {
                // Validate field value type against struct field_types
                if let Some(expected_type) = struct_ref.field_types.get(pos)
                  && !matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic)
                  && !value_matches_type_annotation(&xs[v_idx], expected_type)
                {
                  return CalcitErr::err_str(
                    CalcitErrKind::Type,
                    format!(
                      "&struct:with field `{}` expects type `{}`, but received `{}` ({})",
                      s.ref_str(),
                      expected_type.to_brief_string(),
                      brief_type_of_value(&xs[v_idx]),
                      xs[v_idx].lisp_str()
                    ),
                  );
                }
                xs[v_idx].clone_into(&mut values[pos]);
              }
              None => {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!("&struct:with unexpected field `{s}` for struct: {:?}", struct_ref.fields),
                );
              }
            },
            Calcit::Symbol { sym: s, .. } | Calcit::Str(s) => match struct_value.index_of(s) {
              Some(pos) => {
                // Validate field value type against struct field_types
                if let Some(expected_type) = struct_ref.field_types.get(pos)
                  && !matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic)
                  && !value_matches_type_annotation(&xs[v_idx], expected_type)
                {
                  return CalcitErr::err_str(
                    CalcitErrKind::Type,
                    format!(
                      "&struct:with field `{}` expects type `{}`, but received `{}` ({})",
                      s,
                      expected_type.to_brief_string(),
                      brief_type_of_value(&xs[v_idx]),
                      xs[v_idx].lisp_str()
                    ),
                  );
                }
                xs[v_idx].clone_into(&mut values[pos]);
              }
              None => {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!("&struct:with unexpected field `{s}` for struct: {:?}", struct_ref.fields),
                );
              }
            },
            a => {
              let msg = format!(
                "&struct:with requires field in string/tag, but received: {}",
                type_of(std::slice::from_ref(a))?.lisp_str()
              );
              let hint = format_proc_examples_hint(&CalcitProc::NativeRecordWith).unwrap_or_default();
              return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
            }
          }
        }

        Ok(Calcit::Struct(CalcitStructValue {
          struct_ref: struct_ref.to_owned(),
          values: Arc::new(values),
        }))
      } else {
        CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:with expected pairs, but received:", xs)
      }
    }
    a => {
      let msg = format!(
        "&struct:with requires a struct value as prototype, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordWith).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn get_impls(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let args_size = xs.len();
  if args_size != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:impls expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Struct(struct_value) => Ok(Calcit::from(
      struct_value
        .struct_ref
        .impls
        .iter()
        .map(|x| Calcit::Impl((**x).to_owned()))
        .collect::<Vec<Calcit>>(),
    )),
    Calcit::Enum(enum_value) => Ok(Calcit::from(
      enum_value
        .impls()
        .iter()
        .map(|c| Calcit::Impl((**c).to_owned()))
        .collect::<Vec<_>>(),
    )),
    a => {
      let msg = format!(
        "&struct:impls requires a struct value as prototype, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordImpls).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn struct_from_map(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:from-map expected 2 arguments, but received:", xs);
  }
  // first argument must be a Struct prototype
  let (struct_ref, base_values): (Arc<CalcitStructDef>, Vec<Calcit>) = match &xs[0] {
    Calcit::StructDef(s) => (Arc::new(s.to_owned()), vec![Calcit::Nil; s.fields.len()]),
    a => {
      let msg = format!(
        "&struct:from-map requires a struct as prototype, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordFromMap).unwrap_or_default();
      return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
    }
  };
  match &xs[1] {
    Calcit::Map(ys) => {
      let mut new_values = base_values;
      for (k, v) in ys {
        let key = match k {
          Calcit::Str(s) => s.to_owned(),
          Calcit::Tag(s) => s.ref_str().to_owned().into(),
          a => {
            let msg = format!(
              "&struct:from-map requires field in string/tag, but received: {}",
              type_of(std::slice::from_ref(a))?.lisp_str()
            );
            let hint = format_proc_examples_hint(&CalcitProc::NativeRecordFromMap).unwrap_or_default();
            return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
          }
        };
        match struct_ref.fields.iter().position(|f| f.ref_str() == key.as_ref()) {
          Some(idx) => {
            // Validate field value type against struct field_types
            if let Some(expected_type) = struct_ref.field_types.get(idx)
              && !matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic)
              && !value_matches_type_annotation(v, expected_type)
            {
              return CalcitErr::err_str(
                CalcitErrKind::Type,
                format!(
                  "&struct:from-map field `{}` expects type `{}`, but received `{}` ({})",
                  key,
                  expected_type.to_brief_string(),
                  brief_type_of_value(v),
                  v.lisp_str()
                ),
              );
            }
            new_values[idx] = v.to_owned();
          }
          None => {
            return CalcitErr::err_str(
              CalcitErrKind::Type,
              format!("&struct:from-map invalid field {k} for struct {:?}", struct_ref.fields),
            );
          }
        }
      }
      Ok(Calcit::Struct(CalcitStructValue {
        struct_ref,
        values: Arc::new(new_values),
      }))
    }
    b => {
      let msg = format!(
        "&struct:from-map requires a map as second argument, but received: {}",
        type_of(std::slice::from_ref(b))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordFromMap).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn get_struct_name(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:get-name expected a struct value, but received:", xs);
  }
  match &xs[0] {
    Calcit::Struct(CalcitStructValue { struct_ref, .. }) => Ok(Calcit::Tag(struct_ref.name.to_owned())),
    a => {
      let msg = format!(
        "&struct:get-name requires a struct value, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordGetName).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn get_struct_def(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&struct:definition expected a struct value, but received:",
      xs,
    );
  }
  match &xs[0] {
    Calcit::Struct(value) if value.is_anonymous() => Ok(Calcit::Nil),
    Calcit::Struct(CalcitStructValue { struct_ref, .. }) => Ok(Calcit::StructDef(struct_ref.as_ref().to_owned())),
    a => {
      let msg = format!(
        "&struct:definition requires a struct value, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordStruct).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn turn_map(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:to-map expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Struct(CalcitStructValue { struct_ref, values, .. }) => {
      let mut ys: rpds::HashTrieMapSync<Calcit, Calcit> = rpds::HashTrieMap::new_sync();
      for idx in 0..struct_ref.fields.len() {
        ys.insert_mut(Calcit::Tag(struct_ref.fields[idx].to_owned()), values[idx].to_owned());
      }
      Ok(Calcit::Map(ys))
    }
    a => {
      let msg = format!(
        "&struct:to-map requires a struct value, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordToMap).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn matches(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:matches? expected 2 arguments, but received:", xs);
  }
  // second argument is the target shape to compare against
  let right_struct: &CalcitStructDef = match &xs[1] {
    Calcit::Struct(CalcitStructValue { struct_ref, .. }) => struct_ref,
    Calcit::StructDef(struct_ref) => struct_ref,
    b => {
      let msg = format!(
        "&struct:matches? second argument requires a struct value or struct, but received: {}",
        type_of(std::slice::from_ref(b))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordMatches).unwrap_or_default();
      return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
    }
  };
  match &xs[0] {
    Calcit::Struct(CalcitStructValue {
      struct_ref: left_struct, ..
    }) => Ok(Calcit::Bool(
      left_struct.name == right_struct.name && left_struct.fields == right_struct.fields,
    )),
    a => {
      let msg = format!(
        "&struct:matches? first argument requires a struct value, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordMatches).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn count(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:count expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Struct(CalcitStructValue { struct_ref, .. }) => Ok(Calcit::Number(struct_ref.fields.len() as f64)),
    a => {
      let msg = format!(
        "&struct:count requires a struct value, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordCount).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn contains_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Struct(struct_value)), Some(a)) => match a {
      Calcit::Str(k) | Calcit::Symbol { sym: k, .. } => Ok(Calcit::Bool(struct_value.index_of(k).is_some())),
      Calcit::Tag(k) => Ok(Calcit::Bool(struct_value.index_of(k.ref_str()).is_some())),
      a => {
        let msg = format!(
          "&struct:contains? requires a field in string/tag, but received: {}",
          type_of(std::slice::from_ref(a))?.lisp_str()
        );
        let hint = format_proc_examples_hint(&CalcitProc::NativeRecordContains).unwrap_or_default();
        CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
      }
    },
    (Some(_), None) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordContains).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(
        CalcitErrKind::Arity,
        "&struct:contains? expected 2 arguments, but received:",
        xs,
        hint,
      )
    }
    (Some(a), Some(_)) => {
      let msg = format!(
        "&struct:contains? requires a struct value, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordContains).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (None, ..) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordContains).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(
        CalcitErrKind::Arity,
        "&struct:contains? expected 2 arguments, but received:",
        xs,
        hint,
      )
    }
  }
}

pub fn get(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Struct(struct_value @ CalcitStructValue { values, struct_ref })), Some(a)) => {
      let field_name = match a {
        Calcit::Str(k) | Calcit::Symbol { sym: k, .. } => k.as_ref(),
        Calcit::Tag(k) => k.ref_str(),
        a => {
          let msg = format!(
            "&struct:get requires a field in string/tag, but received: {}",
            type_of(std::slice::from_ref(a))?.lisp_str()
          );
          let hint = format_proc_examples_hint(&CalcitProc::NativeRecordGet).unwrap_or_default();
          return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
        }
      };
      match struct_value.index_of(field_name) {
        Some(idx) => Ok(values[idx].to_owned()),
        None => CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("&struct:get struct `{}` does not define field `:{field_name}`", struct_ref.name),
        ),
      }
    }
    (Some(_), None) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordGet).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(CalcitErrKind::Arity, "&struct:get expected 2 arguments, but received:", xs, hint)
    }
    (Some(a), Some(_)) => {
      let msg = format!(
        "&struct:get requires a struct value, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordGet).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (None, ..) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordGet).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(CalcitErrKind::Arity, "&struct:get expected 2 arguments, but received:", xs, hint)
    }
  }
}

pub fn assoc(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1), xs.get(2)) {
    (Some(Calcit::Struct(struct_value @ CalcitStructValue { struct_ref, values })), Some(a), Some(b)) => match a {
      Calcit::Str(s) | Calcit::Symbol { sym: s, .. } => match struct_value.index_of(s) {
        Some(pos) => {
          let mut new_values = (**values).to_owned();
          b.clone_into(&mut new_values[pos]);
          Ok(Calcit::Struct(CalcitStructValue {
            struct_ref: struct_ref.to_owned(),
            values: Arc::new(new_values),
          }))
        }
        None => CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("&struct:assoc invalid field `{s}` for struct: {:?}", struct_ref.fields),
        ),
      },
      Calcit::Tag(s) => match struct_value.index_of(s.ref_str()) {
        Some(pos) => {
          let mut new_values = (**values).to_owned();
          b.clone_into(&mut new_values[pos]);
          Ok(Calcit::Struct(CalcitStructValue {
            struct_ref: struct_ref.to_owned(),
            values: Arc::new(new_values),
          }))
        }
        None => CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("&struct:assoc invalid field `{s}` for struct: {:?}", struct_ref.fields),
        ),
      },
      a => {
        let msg = format!(
          "&struct:assoc requires a field in string/tag, but received: {} for struct: {:?}",
          type_of(std::slice::from_ref(a))?.lisp_str(),
          struct_ref.fields
        );
        let hint = format_proc_examples_hint(&CalcitProc::NativeRecordAssoc).unwrap_or_default();
        CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
      }
    },
    (Some(_), None, _) | (Some(_), Some(_), None) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordAssoc).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(CalcitErrKind::Arity, "&struct:assoc expected 3 arguments, but received:", xs, hint)
    }
    (Some(a), Some(_), Some(_)) => {
      let msg = format!(
        "&struct:assoc requires a struct value, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordAssoc).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (None, ..) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordAssoc).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(CalcitErrKind::Arity, "&struct:assoc expected 3 arguments, but received:", xs, hint)
    }
  }
}

pub fn extend_as(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 4 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:extend-as expected 4 arguments, but received:", xs);
  }
  match (xs.first(), xs.get(1), xs.get(2), xs.get(3)) {
    (Some(Calcit::Struct(struct_value)), Some(n), Some(a), Some(new_value)) => match a {
      Calcit::Str(s) | Calcit::Symbol { sym: s, .. } => match struct_value.index_of(s) {
        Some(_pos) => CalcitErr::err_str(CalcitErrKind::Unexpected, format!("&struct:extend-as field `{s}` already existed")),
        None => match struct_value.extend_field(&EdnTag(s.to_owned()), n, new_value) {
          Ok(new_struct) => Ok(Calcit::Struct(new_struct)),
          Err(e) => Err(CalcitErr::use_str(CalcitErrKind::Unexpected, e)),
        },
      },
      Calcit::Tag(s) => match struct_value.index_of(s.ref_str()) {
        Some(_pos) => CalcitErr::err_str(CalcitErrKind::Unexpected, format!("&struct:extend-as field `{s}` already existed")),
        None => match struct_value.extend_field(s, n, new_value) {
          Ok(new_struct) => Ok(Calcit::Struct(new_struct)),
          Err(e) => Err(CalcitErr::use_str(CalcitErrKind::Unexpected, e)),
        },
      },
      a => {
        let msg = format!(
          "&struct:extend-as requires a field in string/tag, but received: {} for struct: {:?}",
          type_of(std::slice::from_ref(a))?.lisp_str(),
          struct_value.struct_ref.fields
        );
        let hint = format_proc_examples_hint(&CalcitProc::NativeRecordExtendAs).unwrap_or_default();
        CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
      }
    },
    (Some(a), ..) => {
      let msg = format!(
        "&struct:extend-as requires a struct value, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordExtendAs).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (None, ..) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:extend-as expected 4 arguments, but received:", xs),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitGenericBound, CalcitTrait};

  fn dyn_fn_trait(name: &str, methods: &[&str]) -> CalcitTrait {
    CalcitTrait::new_runtime(
      EdnTag::new(name),
      methods.iter().map(|method| EdnTag::new(*method)).collect(),
      methods.iter().map(|_| Arc::new(CalcitTypeAnnotation::DynFn)).collect(),
    )
  }

  fn impl_pair(name: &str, value: Calcit) -> Calcit {
    Calcit::from(vec![Calcit::tag(name), value])
  }

  fn callable_proc() -> Calcit {
    Calcit::Proc(CalcitProc::NativeResetGenSymIndex)
  }

  fn indexed_struct_fixture() -> Calcit {
    Calcit::Struct(CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(
        EdnTag::new("Point"),
        vec![EdnTag::new("x"), EdnTag::new("y")],
      )),
      values: Arc::new(vec![Calcit::Number(1.0), Calcit::Number(2.0)]),
    })
  }

  #[test]
  fn required_recursive_field_returns_a_type_error_without_recursing() {
    let struct_def = CalcitStructDef {
      name: EdnTag::new("RequiredNode"),
      fields: Arc::new(vec![EdnTag::new("next")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from("RequiredNode"),
        Arc::new(vec![]),
      ))]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    };

    let err = call_struct(&[Calcit::StructDef(struct_def), Calcit::tag("next"), Calcit::Nil])
      .expect_err("a required recursive field must reject nil instead of recursing");

    assert!(err.msg.contains("expects type `'RequiredNode`"), "unexpected error: {err:?}");
  }

  #[test]
  fn struct_get_rejects_missing_fields_instead_of_returning_nil() {
    let struct_value = indexed_struct_fixture();
    let err = get(&[struct_value, Calcit::tag("missing")]).expect_err("missing struct field must not become nil");

    assert_eq!(err.kind, CalcitErrKind::Type);
    assert!(err.msg.contains("does not define field `:missing`"), "unexpected error: {err:?}");
  }

  #[test]
  fn indexed_struct_updates_reject_stale_field_tags() {
    let struct_value = indexed_struct_fixture();
    let assoc_error = struct_assoc_at(&[struct_value.clone(), Calcit::Number(0.0), Calcit::tag("y"), Calcit::Number(3.0)])
      .expect_err("stale assoc tag must fail");
    assert!(assoc_error.msg.contains("expects field `:x`"));

    let with_error = struct_with_at(&[struct_value, Calcit::Number(1.0), Calcit::tag("x"), Calcit::Number(3.0)])
      .expect_err("stale with tag must fail");
    assert!(with_error.msg.contains("expects field `:y`"));
  }

  #[test]
  fn indexed_struct_updates_validate_indices_and_apply_matching_tags() {
    let struct_value = indexed_struct_fixture();
    let invalid_index = struct_assoc_at(&[struct_value.clone(), Calcit::Number(-1.0), Calcit::tag("x"), Calcit::Number(3.0)])
      .expect_err("negative index must fail");
    assert!(invalid_index.msg.contains("non-negative integer index"));

    let updated =
      struct_assoc_at(&[struct_value, Calcit::Number(0.0), Calcit::tag("x"), Calcit::Number(3.0)]).expect("matching index/tag update");
    let Calcit::Struct(updated) = updated else {
      panic!("updated value must remain a struct");
    };
    assert_eq!(updated.values.as_ref(), &[Calcit::Number(3.0), Calcit::Number(2.0)]);
  }

  #[test]
  fn nominal_impl_requires_the_exact_trait_method_set() {
    let trait_def = dyn_fn_trait("Renderable", &["render", "debug"]);
    let missing =
      new_impl(&[Calcit::Trait(trait_def.clone()), impl_pair("render", callable_proc())]).expect_err("missing trait method must fail");
    assert!(missing.msg.contains("missing methods") && missing.msg.contains("debug"));

    let unexpected = new_impl(&[
      Calcit::Trait(trait_def),
      impl_pair("render", callable_proc()),
      impl_pair("debug", callable_proc()),
      impl_pair("extra", callable_proc()),
    ])
    .expect_err("unexpected trait method must fail");
    assert!(unexpected.msg.contains("not declared") && unexpected.msg.contains("extra"));
  }

  #[test]
  fn nominal_impl_rejects_non_callable_method_values() {
    let err = new_impl(&[
      Calcit::Trait(dyn_fn_trait("Renderable", &["render"])),
      impl_pair("render", Calcit::Nil),
    ])
    .expect_err("non-callable trait method must fail");

    assert!(err.msg.contains("expects trait method .render to be a function"));
  }

  #[test]
  fn nominal_impl_checks_available_method_signatures() {
    let trait_def = CalcitTrait::new_runtime(
      EdnTag::new("Renderable"),
      vec![EdnTag::new("render")],
      vec![Arc::new(CalcitTypeAnnotation::from_function_parts(
        vec![Arc::new(CalcitTypeAnnotation::Number)],
        crate::calcit::DYNAMIC_TYPE.clone(),
      ))],
    );
    let err =
      new_impl(&[Calcit::Trait(trait_def), impl_pair("render", callable_proc())]).expect_err("wrong callable signature must fail");

    assert!(err.msg.contains("does not match trait Renderable signature"));
  }

  #[test]
  fn inherent_method_bag_can_be_promoted_to_a_nominal_impl() {
    let method_bag = new_impl(&[Calcit::tag("legacy-methods"), impl_pair("render", callable_proc())]).expect("originless method bag");
    let trait_def = dyn_fn_trait("Renderable", &["render"]);
    let promoted = new_impl(&[Calcit::Trait(trait_def.clone()), method_bag]).expect("promoted nominal impl");
    let Calcit::Impl(promoted) = promoted else {
      panic!("expected impl value");
    };

    assert!(promoted.implements_trait(&trait_def));
    assert!(!promoted.is_inherent());
    assert!(promoted.get("render").is_some());
  }

  fn shown_box_struct(show_trait: Arc<CalcitTrait>) -> CalcitStructDef {
    CalcitStructDef {
      name: EdnTag::new("ShownBox"),
      fields: Arc::new(vec![EdnTag::new("value")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))]),
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![CalcitGenericBound {
        name: Arc::from("T"),
        traits: Arc::new(vec![show_trait]),
      }]),
      impls: vec![],
    }
  }

  fn value_with_trait(trait_def: Arc<CalcitTrait>) -> Calcit {
    let mut struct_def = CalcitStructDef::from_fields(EdnTag::new("ShownValue"), vec![]);
    struct_def.impls.push(Arc::new(CalcitImpl {
      name: trait_def.name.clone(),
      origin: Some(trait_def),
      fields: Arc::new(vec![]),
      values: Arc::new(vec![]),
    }));
    Calcit::Struct(CalcitStructValue {
      struct_ref: Arc::new(struct_def),
      values: Arc::new(vec![]),
    })
  }

  #[test]
  fn generic_struct_where_bounds_accept_nominal_trait_values() {
    let show_trait = Arc::new(CalcitTrait::new(EdnTag::new("Renderable"), vec![], vec![]));
    let result = call_struct(&[
      Calcit::StructDef(shown_box_struct(show_trait.clone())),
      Calcit::tag("value"),
      value_with_trait(show_trait),
    ]);

    assert!(result.is_ok(), "expected shown box creation to pass: {result:?}");
  }

  #[test]
  fn generic_struct_where_bounds_reject_missing_nominal_trait() {
    let show_trait = Arc::new(CalcitTrait::new(EdnTag::new("Renderable"), vec![], vec![]));
    let err = call_struct(&[
      Calcit::StructDef(shown_box_struct(show_trait)),
      Calcit::tag("value"),
      Calcit::Proc(CalcitProc::NativeResetGenSymIndex),
    ])
    .expect_err("expected shown box creation to fail on non-Show payload");

    assert!(
      err.msg.contains("does not satisfy `trait Renderable`") || err.msg.contains("does not satisfy `Renderable`"),
      "unexpected error: {err:?}"
    );
  }
}
