use std::ops::Rem;
use std::sync::Arc;

use cirru_edn::EdnTag;

use crate::builtins::meta::type_of;
use crate::calcit::{
  Calcit, CalcitEnum, CalcitErr, CalcitErrKind, CalcitImpl, CalcitList, CalcitProc, CalcitRecord, CalcitStruct, CalcitSyntax,
  CalcitTypeAnnotation, brief_type_of_value, format_proc_examples_hint, value_matches_type_annotation,
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

  let mut vars = Vec::with_capacity(items.len());
  for item in items.iter() {
    let name = parse_type_var_form(item)?;
    vars.push(name);
  }
  Some(vars)
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
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeImplNew).unwrap_or_default();
      return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
    }
  };

  if xs.len() == 1 {
    return Ok(Calcit::Impl(CalcitImpl {
      name: name_id,
      origin,
      fields: Arc::new(vec![]),
      values: Arc::new(vec![]),
    }));
  }

  let mut entries: Vec<(EdnTag, Calcit)> = Vec::with_capacity(xs.len().saturating_sub(1));
  for item in xs.iter().skip(1) {
    let (name, value) = match item {
      Calcit::List(pair) => match (pair.first(), pair.get(1), pair.get(2)) {
        (Some(name), Some(value), None) => (name, value),
        _ => {
          let msg = format!("&impl::new expects (field value) pairs, but received: {item}");
          let hint = format_proc_examples_hint(&CalcitProc::NativeImplNew).unwrap_or_default();
          return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
        }
      },
      Calcit::Tuple(tuple) => match (tuple.extra.first(), tuple.extra.get(1)) {
        (Some(value), None) => (tuple.tag.as_ref(), value),
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
      "&struct::new expects a name and field definitions, but received none:",
      xs,
      hint,
    );
  }

  let name_id: EdnTag = match &xs[0] {
    Calcit::Symbol { sym, .. } => EdnTag(sym.to_owned()),
    Calcit::Tag(k) => k.to_owned(),
    a => {
      let msg = format!(
        "&struct::new expects a name (symbol or tag), but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
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

  let mut fields: Vec<(EdnTag, Arc<CalcitTypeAnnotation>)> = vec![];
  for item in xs.iter().skip(start_idx) {
    match item {
      Calcit::List(xs) => match (xs.first(), xs.get(1), xs.get(2)) {
        (Some(name), Some(type_expr), None) => {
          let field_name = match name {
            Calcit::Symbol { sym, .. } | Calcit::Str(sym) => EdnTag(sym.to_owned()),
            Calcit::Tag(tag) => tag.to_owned(),
            other => {
              let msg = format!("&struct::new field expects a tag/symbol, but received: {other}");
              let hint = format_proc_examples_hint(&CalcitProc::NativeStructNew).unwrap_or_default();
              return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
            }
          };
          let field_type = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(type_expr, generics.as_slice());
          if let Err(e) = field_type.validate_applied_type_args() {
            let hint = format_proc_examples_hint(&CalcitProc::NativeStructNew).unwrap_or_default();
            return CalcitErr::err_str_with_hint(
              CalcitErrKind::Type,
              format!("&struct::new field `{field_name}` has invalid type annotation: {e}"),
              hint,
            );
          }
          fields.push((field_name, field_type));
        }
        (Some(_), None, _) => {
          let hint = format_proc_examples_hint(&CalcitProc::NativeStructNew).unwrap_or_default();
          return CalcitErr::err_str_with_hint(
            CalcitErrKind::Arity,
            "&struct::new field expects a pair (field type), but received only a field name",
            hint,
          );
        }
        _ => {
          let msg = format!("&struct::new field expects a pair list, but received: {item}");
          let hint = format_proc_examples_hint(&CalcitProc::NativeStructNew).unwrap_or_default();
          return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
        }
      },
      other => {
        let msg = format!("&struct::new expects field entries as lists, but received: {other}");
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
        format!("&struct::new duplicated field: {}", fields[idx].0),
      );
    }
  }

  generics.sort();
  generics.dedup();

  let field_names: Vec<EdnTag> = fields.iter().map(|(name, _)| name.to_owned()).collect();
  let field_types: Vec<Arc<CalcitTypeAnnotation>> = fields.iter().map(|(_, t)| t.to_owned()).collect();

  Ok(Calcit::Struct(CalcitStruct {
    name: name_id,
    fields: Arc::new(field_names),
    field_types: Arc::new(field_types),
    generics: Arc::new(generics),
    impls: vec![],
  }))
}

pub fn new_enum(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    let hint = format_proc_examples_hint(&CalcitProc::NativeEnumNew).unwrap_or_default();
    return CalcitErr::err_nodes_with_hint(
      CalcitErrKind::Arity,
      "&enum::new expects a name and variants, but received none:",
      xs,
      hint,
    );
  }

  let name_id: EdnTag = match &xs[0] {
    Calcit::Symbol { sym, .. } => EdnTag(sym.to_owned()),
    Calcit::Tag(k) => k.to_owned(),
    a => {
      let msg = format!(
        "&enum::new expects a name (symbol or tag), but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeEnumNew).unwrap_or_default();
      return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
    }
  };

  let mut variants: Vec<(EdnTag, Calcit)> = vec![];
  for item in xs.iter().skip(1) {
    match item {
      Calcit::List(xs) => {
        let tag = match xs.first() {
          Some(Calcit::Symbol { sym, .. }) => EdnTag(sym.to_owned()),
          Some(Calcit::Tag(k)) => k.to_owned(),
          Some(other) => {
            let msg = format!("&enum::new variant expects a tag, but received: {other}");
            let hint = format_proc_examples_hint(&CalcitProc::NativeEnumNew).unwrap_or_default();
            return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
          }
          None => {
            let hint = format_proc_examples_hint(&CalcitProc::NativeEnumNew).unwrap_or_default();
            return CalcitErr::err_str_with_hint(
              CalcitErrKind::Arity,
              "&enum::new variant expects a tag and payload types, but received an empty list",
              hint,
            );
          }
        };

        let payloads = xs.drop_left();
        let payload_list = Calcit::List(Arc::new(CalcitList::Vector(payloads.to_vec())));
        variants.push((tag, payload_list));
      }
      other => {
        let msg = format!("&enum::new expects variants as lists, but received: {other}");
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
        format!("&enum::new duplicated variant: {}", variants[idx].0),
      );
    }
  }

  let fields: Vec<EdnTag> = variants.iter().map(|(tag, _)| tag.to_owned()).collect();
  let values: Vec<Calcit> = variants.iter().map(|(_, value)| value.to_owned()).collect();

  let mut struct_ref = CalcitStruct::from_fields(name_id, fields);
  struct_ref.impls = vec![Arc::new(enum_prototype_marker())];

  let record = CalcitRecord {
    struct_ref: Arc::new(struct_ref),
    values: Arc::new(values),
  };

  match CalcitEnum::from_record(record) {
    Ok(enum_def) => Ok(Calcit::Enum(enum_def)),
    Err(msg) => CalcitErr::err_str(CalcitErrKind::Type, format!("&enum::new failed to build enum: {msg}")),
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

/// Partial record constructor — missing fields default to nil.
/// Proto must be a `Calcit::Struct` from `defstruct`.
pub fn call_record_partial(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let args_size = xs.len();
  if args_size < 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&%{}? expected at least 1 argument, but received:", xs);
  }
  if (args_size - 1).rem(2) != 0 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&%{}? expected pairs after prototype, but received:", xs);
  }
  let (base_struct, mut base_values): (Arc<CalcitStruct>, Vec<Calcit>) = match &xs[0] {
    Calcit::Struct(s) => {
      let vals = vec![Calcit::Nil; s.fields.len()];
      (Arc::new(s.to_owned()), vals)
    }
    a => {
      return CalcitErr::err_str(
        CalcitErrKind::Type,
        format!(
          "&%{{}}? requires a struct as prototype, but received: {}",
          type_of(&[a.to_owned()])?.lisp_str()
        ),
      );
    }
  };
  let size = (args_size - 1) / 2;
  let mut seen_positions: Vec<bool> = vec![false; base_struct.fields.len()];
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
            type_of(&[a.to_owned()])?.lisp_str()
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
        }
        xs[v_idx].clone_into(&mut base_values[pos]);
      }
      None => {
        return CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("&%{{}}? unexpected field `{field_name}` for record: {:?}", base_struct.fields),
        );
      }
    }
  }
  Ok(Calcit::Record(CalcitRecord {
    struct_ref: base_struct,
    values: Arc::new(base_values),
  }))
}

/// Create a loose record from key-value pairs: `?{} :field1 val1 :field2 val2`
/// Fields are sorted alphabetically, mirroring struct-backed record behaviour.
pub fn call_loose_record(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
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
  Ok(Calcit::Record(CalcitRecord::from_loose_pairs(fields, values)))
}

/// Direct indexed access to a record field: `&record:nth record index`
/// This is the optimized path emitted by the preprocessor when the field index is known at compile time.
pub fn record_nth(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  // Accept 2 or 3 args: (record, idx) or (record, idx, :field-tag)
  // The 3rd arg (field tag) is only used by JS codegen; Rust runtime ignores it.
  if xs.len() < 2 || xs.len() > 3 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:nth expected 2-3 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Record(CalcitRecord { values, struct_ref }), Calcit::Number(n)) => {
      let idx = *n as usize;
      if idx < values.len() {
        Ok(values[idx].to_owned())
      } else {
        CalcitErr::err_str(
          CalcitErrKind::Arity,
          format!(
            "&record:nth index {} out of range for record `{}` with {} fields",
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
        "&record:nth expected (record, number), but received: {} {}",
        a.lisp_str(),
        b.lisp_str()
      ),
    ),
  }
}

/// Direct indexed assoc on a record field: `&record:assoc-at record index :field value`
/// This is the optimized path emitted by the preprocessor when the field index is known at compile time.
pub fn record_assoc_at(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  // 4 args: (record, idx, :field-tag, value)
  // The 3rd arg (field tag) is only used by JS codegen; Rust runtime ignores it.
  if xs.len() != 4 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:assoc-at expected 4 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Record(CalcitRecord { struct_ref, values }), Calcit::Number(n)) => {
      let idx = *n as usize;
      if idx < values.len() {
        let mut new_values = (**values).to_owned();
        xs[3].clone_into(&mut new_values[idx]);
        Ok(Calcit::Record(CalcitRecord {
          struct_ref: struct_ref.to_owned(),
          values: Arc::new(new_values),
        }))
      } else {
        CalcitErr::err_str(
          CalcitErrKind::Arity,
          format!(
            "&record:assoc-at index {} out of range for record `{}` with {} fields",
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
        "&record:assoc-at expected (record, number), but received: {} {}",
        a.lisp_str(),
        b.lisp_str()
      ),
    ),
  }
}

/// Optimized `&record:with` — field indices pre-resolved at compile time.
/// Args: (record, idx1, :tag1, val1, idx2, :tag2, val2, ...)
/// Tags are carried for JS codegen; Rust runtime uses indices directly.
pub fn record_with_at(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() || (xs.len() - 1) % 3 != 0 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&record:with-at expected (record, idx, tag, val, ...) triples, but received:",
      xs,
    );
  }
  match &xs[0] {
    Calcit::Record(CalcitRecord { struct_ref, values }) => {
      let mut new_values = (**values).to_owned();
      let triple_count = (xs.len() - 1) / 3;
      for i in 0..triple_count {
        let base = 1 + i * 3;
        match &xs[base] {
          Calcit::Number(n) => {
            let idx = *n as usize;
            if idx < new_values.len() {
              xs[base + 2].clone_into(&mut new_values[idx]);
            } else {
              return CalcitErr::err_str(
                CalcitErrKind::Arity,
                format!(
                  "&record:with-at index {} out of range for record `{}` with {} fields",
                  idx,
                  struct_ref.name,
                  new_values.len()
                ),
              );
            }
          }
          other => {
            return CalcitErr::err_str(
              CalcitErrKind::Type,
              format!("&record:with-at expected number index, but received: {}", other.lisp_str()),
            );
          }
        }
      }
      Ok(Calcit::Record(CalcitRecord {
        struct_ref: struct_ref.to_owned(),
        values: Arc::new(new_values),
      }))
    }
    a => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&record:with-at expected a record, but received: {}", a.lisp_str()),
    ),
  }
}

pub fn call_record(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let args_size = xs.len();
  if args_size < 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&%{{}} expected at least 2 arguments, but received:", xs);
  }
  match &xs[0] {
    Calcit::Struct(struct_def) => {
      let record = CalcitRecord {
        struct_ref: Arc::new(struct_def.to_owned()),
        values: Arc::new(vec![Calcit::Nil; struct_def.fields.len()]),
      };
      call_record_with_prototype(&record, xs)
    }
    Calcit::Record(_) => CalcitErr::err_str(
      CalcitErrKind::Type,
      "&%{} requires a struct (from defstruct) as prototype, not a record instance; use defstruct to define the type",
    ),
    a => {
      let msg = format!(
        "&%{{}} requires a struct as prototype, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecord).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

fn call_record_with_prototype(record: &CalcitRecord, xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let args_size = xs.len();
  let CalcitRecord { struct_ref, values: v0 } = record;
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

  for idx in 0..size {
    let k_idx = idx * 2 + 1;
    let v_idx = k_idx + 1;
    match &xs[k_idx] {
      Calcit::Tag(s) => match record.index_of(s.ref_str()) {
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
          }
          xs[v_idx].clone_into(&mut values[pos]);
        }
        None => {
          return CalcitErr::err_str(
            CalcitErrKind::Type,
            format!("&%{{}} unexpected field `{s}` for record: {:?}", struct_ref.fields),
          );
        }
      },
      Calcit::Symbol { sym: s, .. } | Calcit::Str(s) => match record.index_of(s) {
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
          }
          xs[v_idx].clone_into(&mut values[pos]);
        }
        None => {
          return CalcitErr::err_str(
            CalcitErrKind::Type,
            format!("&%{{}} unexpected field `{s}` for record: {:?}", struct_ref.fields),
          );
        }
      },
      a => {
        let msg = format!(
          "&%{{}} requires field in string/tag, but received: {}",
          type_of(&[a.to_owned()])?.lisp_str()
        );
        let hint = format_proc_examples_hint(&CalcitProc::NativeRecord).unwrap_or_default();
        return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
      }
    }
  }

  Ok(Calcit::Record(CalcitRecord {
    struct_ref: struct_ref.to_owned(),
    values: Arc::new(values),
  }))
}

/// takes a record and pairs of key value(flatterned), and update the record. raise error if key not existed in the record
pub fn record_with(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let args_size = xs.len();
  if args_size < 3 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&record:with expected at least 3 arguments, but received:",
      xs,
    );
  }
  match &xs[0] {
    Calcit::Record(record @ CalcitRecord { struct_ref, values: v0 }) => {
      if (args_size - 1).rem(2) == 0 {
        let size = (args_size - 1) / 2;
        let mut values: Vec<Calcit> = (**v0).to_owned();

        for idx in 0..size {
          let k_idx = idx * 2 + 1;
          let v_idx = k_idx + 1;
          match &xs[k_idx] {
            Calcit::Tag(s) => match record.index_of(s.ref_str()) {
              Some(pos) => {
                // Validate field value type against struct field_types
                if let Some(expected_type) = struct_ref.field_types.get(pos) {
                  if !matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic)
                    && !value_matches_type_annotation(&xs[v_idx], expected_type)
                  {
                    return CalcitErr::err_str(
                      CalcitErrKind::Type,
                      format!(
                        "&record:with field `{}` expects type `{}`, but received `{}` ({})",
                        s.ref_str(),
                        expected_type.to_brief_string(),
                        brief_type_of_value(&xs[v_idx]),
                        xs[v_idx].lisp_str()
                      ),
                    );
                  }
                }
                xs[v_idx].clone_into(&mut values[pos]);
              }
              None => {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!("&record:with unexpected field `{s}` for record: {:?}", struct_ref.fields),
                );
              }
            },
            Calcit::Symbol { sym: s, .. } | Calcit::Str(s) => match record.index_of(s) {
              Some(pos) => {
                // Validate field value type against struct field_types
                if let Some(expected_type) = struct_ref.field_types.get(pos) {
                  if !matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic)
                    && !value_matches_type_annotation(&xs[v_idx], expected_type)
                  {
                    return CalcitErr::err_str(
                      CalcitErrKind::Type,
                      format!(
                        "&record:with field `{}` expects type `{}`, but received `{}` ({})",
                        s,
                        expected_type.to_brief_string(),
                        brief_type_of_value(&xs[v_idx]),
                        xs[v_idx].lisp_str()
                      ),
                    );
                  }
                }
                xs[v_idx].clone_into(&mut values[pos]);
              }
              None => {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!("&record:with unexpected field `{s}` for record: {:?}", struct_ref.fields),
                );
              }
            },
            a => {
              let msg = format!(
                "&record:with requires field in string/tag, but received: {}",
                type_of(&[a.to_owned()])?.lisp_str()
              );
              let hint = format_proc_examples_hint(&CalcitProc::NativeRecordWith).unwrap_or_default();
              return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
            }
          }
        }

        Ok(Calcit::Record(CalcitRecord {
          struct_ref: struct_ref.to_owned(),
          values: Arc::new(values),
        }))
      } else {
        CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:with expected pairs, but received:", xs)
      }
    }
    a => {
      let msg = format!(
        "&record:with requires a record as prototype, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordWith).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn get_impls(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let args_size = xs.len();
  if args_size != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:impls expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Record(record) => Ok(Calcit::from(
      record
        .struct_ref
        .impls
        .iter()
        .map(|x| Calcit::Impl((**x).to_owned()))
        .collect::<Vec<Calcit>>(),
    )),
    Calcit::Tuple(tuple) => Ok(Calcit::from(
      tuple.impls().iter().map(|c| Calcit::Impl((**c).to_owned())).collect::<Vec<_>>(),
    )),
    a => {
      let msg = format!(
        "&record:impls requires a record as prototype, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordImpls).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn record_from_map(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:from-map expected 2 arguments, but received:", xs);
  }
  // first argument must be a Struct prototype
  let (struct_ref, base_values): (Arc<CalcitStruct>, Vec<Calcit>) = match &xs[0] {
    Calcit::Struct(s) => (Arc::new(s.to_owned()), vec![Calcit::Nil; s.fields.len()]),
    a => {
      let msg = format!(
        "&record:from-map requires a struct as prototype, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
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
              "&record:from-map requires field in string/tag, but received: {}",
              type_of(&[a.to_owned()])?.lisp_str()
            );
            let hint = format_proc_examples_hint(&CalcitProc::NativeRecordFromMap).unwrap_or_default();
            return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
          }
        };
        match struct_ref.fields.iter().position(|f| f.ref_str() == key.as_ref()) {
          Some(idx) => {
            // Validate field value type against struct field_types
            if let Some(expected_type) = struct_ref.field_types.get(idx) {
              if !matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic) && !value_matches_type_annotation(v, expected_type) {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!(
                    "&record:from-map field `{}` expects type `{}`, but received `{}` ({})",
                    key,
                    expected_type.to_brief_string(),
                    brief_type_of_value(v),
                    v.lisp_str()
                  ),
                );
              }
            }
            new_values[idx] = v.to_owned();
          }
          None => {
            return CalcitErr::err_str(
              CalcitErrKind::Type,
              format!("&record:from-map invalid field {k} for record {:?}", struct_ref.fields),
            );
          }
        }
      }
      Ok(Calcit::Record(CalcitRecord {
        struct_ref,
        values: Arc::new(new_values),
      }))
    }
    b => {
      let msg = format!(
        "&record:from-map requires a map as second argument, but received: {}",
        type_of(&[b.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordFromMap).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn get_record_name(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:get-name expected a record, but received:", xs);
  }
  match &xs[0] {
    Calcit::Record(CalcitRecord { struct_ref, .. }) => Ok(Calcit::Tag(struct_ref.name.to_owned())),
    a => {
      let msg = format!(
        "&record:get-name requires a record, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordGetName).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn get_record_struct(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:struct expected a record, but received:", xs);
  }
  match &xs[0] {
    Calcit::Record(CalcitRecord { struct_ref, .. }) => Ok(Calcit::Struct(struct_ref.as_ref().to_owned())),
    a => {
      let msg = format!(
        "&record:struct requires a record, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordStruct).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn turn_map(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:to-map expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Record(CalcitRecord { struct_ref, values, .. }) => {
      let mut ys: rpds::HashTrieMapSync<Calcit, Calcit> = rpds::HashTrieMap::new_sync();
      for idx in 0..struct_ref.fields.len() {
        ys.insert_mut(Calcit::Tag(struct_ref.fields[idx].to_owned()), values[idx].to_owned());
      }
      Ok(Calcit::Map(ys))
    }
    a => {
      let msg = format!(
        "&record:to-map requires a record, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordToMap).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}
pub fn matches(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:matches? expected 2 arguments, but received:", xs);
  }
  // second argument is the target shape to compare against
  let right_struct: &CalcitStruct = match &xs[1] {
    Calcit::Record(CalcitRecord { struct_ref, .. }) => struct_ref,
    Calcit::Struct(struct_ref) => struct_ref,
    b => {
      let msg = format!(
        "&record:matches? second argument requires a record or struct, but received: {}",
        type_of(&[b.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordMatches).unwrap_or_default();
      return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
    }
  };
  match &xs[0] {
    Calcit::Record(CalcitRecord {
      struct_ref: left_struct, ..
    }) => Ok(Calcit::Bool(
      left_struct.name == right_struct.name && left_struct.fields == right_struct.fields,
    )),
    a => {
      let msg = format!(
        "&record:matches? first argument requires a record, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordMatches).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn count(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:count expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Record(CalcitRecord { struct_ref, .. }) => Ok(Calcit::Number(struct_ref.fields.len() as f64)),
    a => {
      let msg = format!(
        "&record:count requires a record, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordCount).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn contains_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Record(record)), Some(a)) => match a {
      Calcit::Str(k) | Calcit::Symbol { sym: k, .. } => Ok(Calcit::Bool(record.index_of(k).is_some())),
      Calcit::Tag(k) => Ok(Calcit::Bool(record.index_of(k.ref_str()).is_some())),
      a => {
        let msg = format!(
          "&record:contains? requires a field in string/tag, but received: {}",
          type_of(&[a.to_owned()])?.lisp_str()
        );
        let hint = format_proc_examples_hint(&CalcitProc::NativeRecordContains).unwrap_or_default();
        CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
      }
    },
    (Some(_), None) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordContains).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(
        CalcitErrKind::Arity,
        "&record:contains? expected 2 arguments, but received:",
        xs,
        hint,
      )
    }
    (Some(a), Some(_)) => {
      let msg = format!(
        "&record:contains? requires a record, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordContains).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (None, ..) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordContains).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(
        CalcitErrKind::Arity,
        "&record:contains? expected 2 arguments, but received:",
        xs,
        hint,
      )
    }
  }
}

pub fn get(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Record(record @ CalcitRecord { values, .. })), Some(a)) => match a {
      Calcit::Str(k) | Calcit::Symbol { sym: k, .. } => match record.index_of(k) {
        Some(idx) => Ok(values[idx].to_owned()),
        None => Ok(Calcit::Nil),
      },
      Calcit::Tag(k) => match record.index_of(k.ref_str()) {
        Some(idx) => Ok(values[idx].to_owned()),
        None => Ok(Calcit::Nil),
      },
      a => {
        let msg = format!(
          "&record:get requires a field in string/tag, but received: {}",
          type_of(&[a.to_owned()])?.lisp_str()
        );
        let hint = format_proc_examples_hint(&CalcitProc::NativeRecordGet).unwrap_or_default();
        CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
      }
    },
    (Some(_), None) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordGet).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(CalcitErrKind::Arity, "&record:get expected 2 arguments, but received:", xs, hint)
    }
    (Some(a), Some(_)) => {
      let msg = format!(
        "&record:get requires a record, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordGet).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (None, ..) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordGet).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(CalcitErrKind::Arity, "&record:get expected 2 arguments, but received:", xs, hint)
    }
  }
}

pub fn assoc(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1), xs.get(2)) {
    (Some(Calcit::Record(record @ CalcitRecord { struct_ref, values })), Some(a), Some(b)) => match a {
      Calcit::Str(s) | Calcit::Symbol { sym: s, .. } => match record.index_of(s) {
        Some(pos) => {
          let mut new_values = (**values).to_owned();
          b.clone_into(&mut new_values[pos]);
          Ok(Calcit::Record(CalcitRecord {
            struct_ref: struct_ref.to_owned(),
            values: Arc::new(new_values),
          }))
        }
        None => CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("&record:assoc invalid field `{s}` for record: {:?}", struct_ref.fields),
        ),
      },
      Calcit::Tag(s) => match record.index_of(s.ref_str()) {
        Some(pos) => {
          let mut new_values = (**values).to_owned();
          b.clone_into(&mut new_values[pos]);
          Ok(Calcit::Record(CalcitRecord {
            struct_ref: struct_ref.to_owned(),
            values: Arc::new(new_values),
          }))
        }
        None => CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("&record:assoc invalid field `{s}` for record: {:?}", struct_ref.fields),
        ),
      },
      a => {
        let msg = format!(
          "&record:assoc requires a field in string/tag, but received: {} for record: {:?}",
          type_of(&[a.to_owned()])?.lisp_str(),
          struct_ref.fields
        );
        let hint = format_proc_examples_hint(&CalcitProc::NativeRecordAssoc).unwrap_or_default();
        CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
      }
    },
    (Some(_), None, _) | (Some(_), Some(_), None) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordAssoc).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(CalcitErrKind::Arity, "&record:assoc expected 3 arguments, but received:", xs, hint)
    }
    (Some(a), Some(_), Some(_)) => {
      let msg = format!(
        "&record:assoc requires a record, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordAssoc).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (None, ..) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordAssoc).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(CalcitErrKind::Arity, "&record:assoc expected 3 arguments, but received:", xs, hint)
    }
  }
}

pub fn extend_as(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 4 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:extend-as expected 4 arguments, but received:", xs);
  }
  match (xs.first(), xs.get(1), xs.get(2), xs.get(3)) {
    (Some(Calcit::Record(record)), Some(n), Some(a), Some(new_value)) => match a {
      Calcit::Str(s) | Calcit::Symbol { sym: s, .. } => match record.index_of(s) {
        Some(_pos) => CalcitErr::err_str(CalcitErrKind::Unexpected, format!("&record:extend-as field `{s}` already existed")),
        None => match record.extend_field(&EdnTag(s.to_owned()), n, new_value) {
          Ok(new_record) => Ok(Calcit::Record(new_record)),
          Err(e) => Err(CalcitErr::use_str(CalcitErrKind::Unexpected, e)),
        },
      },
      Calcit::Tag(s) => match record.index_of(s.ref_str()) {
        Some(_pos) => CalcitErr::err_str(CalcitErrKind::Unexpected, format!("&record:extend-as field `{s}` already existed")),
        None => match record.extend_field(s, n, new_value) {
          Ok(new_record) => Ok(Calcit::Record(new_record)),
          Err(e) => Err(CalcitErr::use_str(CalcitErrKind::Unexpected, e)),
        },
      },
      a => {
        let msg = format!(
          "&record:extend-as requires a field in string/tag, but received: {} for record: {:?}",
          type_of(&[a.to_owned()])?.lisp_str(),
          record.struct_ref.fields
        );
        let hint = format_proc_examples_hint(&CalcitProc::NativeRecordExtendAs).unwrap_or_default();
        CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
      }
    },
    (Some(a), ..) => {
      let msg = format!(
        "&record:extend-as requires a record, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordExtendAs).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (None, ..) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:extend-as expected 4 arguments, but received:", xs),
  }
}
