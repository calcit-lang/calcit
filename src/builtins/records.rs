use std::ops::Rem;
use std::sync::Arc;

use cirru_edn::EdnTag;

use crate::builtins::meta::type_of;
use crate::calcit::{Calcit, CalcitErr, CalcitErrKind, CalcitProc, CalcitRecord, CalcitTuple, format_proc_examples_hint};

pub fn new_record(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "new-record expected arguments, but received none:", xs);
  }
  let name_id: EdnTag = match &xs[0] {
    Calcit::Symbol { sym, .. } => EdnTag(sym.to_owned()),
    Calcit::Tag(k) => k.to_owned(),
    a => {
      let msg = format!(
        "new-record requires a name (symbol or tag), but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NewRecord).unwrap_or_default();
      return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
    }
  };

  let mut fields: Vec<EdnTag> = Vec::with_capacity(xs.len());
  let mut values: Vec<Calcit> = Vec::with_capacity(xs.len());

  for x in xs.iter().skip(1) {
    match x {
      Calcit::Symbol { sym, .. } | Calcit::Str(sym) => {
        fields.push(EdnTag(sym.to_owned()));
      }
      Calcit::Tag(s) => {
        fields.push(s.to_owned());
      }
      a => {
        let msg = format!(
          "new-record fields require tag/string, but received: {}",
          type_of(&[a.to_owned()])?.lisp_str()
        );
        let hint = format_proc_examples_hint(&CalcitProc::NewRecord).unwrap_or_default();
        return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
      }
    }
    values.push(Calcit::Nil);
  }
  fields.sort_unstable(); // all values are nil

  // warn about dup
  let mut prev: EdnTag = EdnTag::new(""); // actually a invalid default...
  for (idx, x) in fields.iter().enumerate() {
    if idx > 0 {
      if x == &prev {
        return CalcitErr::err_str(CalcitErrKind::Unexpected, format!("new-record duplicated field: {x}"));
      } else {
        x.clone_into(&mut prev);
        // checked ok
      }
    } else {
      x.clone_into(&mut prev)
    }
  }
  Ok(Calcit::Record(CalcitRecord {
    name: name_id,
    fields: Arc::new(fields),
    values: Arc::new(values),
    class: None,
  }))
}

pub fn new_class_record(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "new-class-record expected arguments, but received none:", xs);
  }
  let class = match &xs[0] {
    Calcit::Record(class) => class.to_owned(),
    b => {
      let msg = format!(
        "new-class-record requires a class (record), but received: {}",
        type_of(&[b.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NewClassRecord).unwrap_or_default();
      return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
    }
  };
  let name_id: EdnTag = match &xs[1] {
    Calcit::Symbol { sym, .. } => EdnTag(sym.to_owned()),
    Calcit::Tag(k) => k.to_owned(),
    a => {
      let msg = format!(
        "new-class-record requires a name (symbol or tag), but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NewClassRecord).unwrap_or_default();
      return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
    }
  };

  let mut fields: Vec<EdnTag> = Vec::with_capacity(xs.len());
  let mut values: Vec<Calcit> = Vec::with_capacity(xs.len());

  for x in xs.iter().skip(2) {
    match x {
      Calcit::Symbol { sym, .. } | Calcit::Str(sym) => {
        fields.push(EdnTag(sym.to_owned()));
      }
      Calcit::Tag(s) => {
        fields.push(s.to_owned());
      }
      a => {
        let msg = format!(
          "new-class-record fields require tag/string, but received: {}",
          type_of(&[a.to_owned()])?.lisp_str()
        );
        let hint = format_proc_examples_hint(&CalcitProc::NewClassRecord).unwrap_or_default();
        return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
      }
    }
    values.push(Calcit::Nil);
  }
  fields.sort_unstable(); // all values are nil

  // warn about dup
  let mut prev: EdnTag = EdnTag::new(""); // actually a invalid default...
  for (idx, x) in fields.iter().enumerate() {
    if idx > 0 {
      if x == &prev {
        return CalcitErr::err_str(CalcitErrKind::Unexpected, format!("new-class-record duplicated field: {x}"));
      } else {
        x.clone_into(&mut prev);
        // checked ok
      }
    } else {
      x.clone_into(&mut prev)
    }
  }
  Ok(Calcit::Record(CalcitRecord {
    name: name_id,
    fields: Arc::new(fields),
    values: Arc::new(values),
    class: Some(Arc::new(class)),
  }))
}

pub fn call_record(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let args_size = xs.len();
  if args_size < 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&%{{}} expected at least 2 arguments, but received:", xs);
  }
  match &xs[0] {
    Calcit::Record(
      record @ CalcitRecord {
        name,
        fields: def_fields,
        values: v0,
        class,
      },
    ) => {
      if (args_size - 1).rem(2) == 0 {
        let size = (args_size - 1) / 2;
        if size != def_fields.len() {
          return CalcitErr::err_str(
            CalcitErrKind::Arity,
            format!(
              "&%{{}} unexpected number of fields. Expected {}, but received {}",
              def_fields.len(),
              size
            ),
          );
        }
        let mut values: Vec<Calcit> = (**v0).to_owned();

        for idx in 0..size {
          let k_idx = idx * 2 + 1;
          let v_idx = k_idx + 1;
          match &xs[k_idx] {
            Calcit::Tag(s) => match record.index_of(s.ref_str()) {
              Some(pos) => {
                xs[v_idx].clone_into(&mut values[pos]);
              }
              None => {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!("&%{{}} unexpected field `{s}` for record: {def_fields:?}"),
                );
              }
            },
            Calcit::Symbol { sym: s, .. } | Calcit::Str(s) => match record.index_of(s) {
              Some(pos) => {
                xs[v_idx].clone_into(&mut values[pos]);
              }
              None => {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!("&%{{}} unexpected field `{s}` for record: {def_fields:?}"),
                );
              }
            },
            a => {
              let msg = format!(
                "&%{{}} requires field in string/tag, but received: {}",
                type_of(&[a.to_owned()])?.lisp_str()
              );
              let hint = format_proc_examples_hint(&CalcitProc::NewRecord).unwrap_or_default();
              return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
            }
          }
        }

        Ok(Calcit::Record(CalcitRecord {
          name: name.to_owned(),
          fields: def_fields.to_owned(),
          values: Arc::new(values),
          class: class.to_owned(),
        }))
      } else {
        CalcitErr::err_nodes(CalcitErrKind::Arity, "&%{{}} expected pairs, but received:", xs)
      }
    }
    a => {
      let msg = format!(
        "&%{{}} requires a record as prototype, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NewRecord).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

/// takes a record and pairs of key value(flatterned), and update the record. raise error if key not existed in the record
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
    Calcit::Record(
      record @ CalcitRecord {
        name,
        fields: def_fields,
        values: v0,
        class,
      },
    ) => {
      if (args_size - 1).rem(2) == 0 {
        let size = (args_size - 1) / 2;
        let mut values: Vec<Calcit> = (**v0).to_owned();

        for idx in 0..size {
          let k_idx = idx * 2 + 1;
          let v_idx = k_idx + 1;
          match &xs[k_idx] {
            Calcit::Tag(s) => match record.index_of(s.ref_str()) {
              Some(pos) => {
                xs[v_idx].clone_into(&mut values[pos]);
              }
              None => {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!("&record:with unexpected field `{s}` for record: {def_fields:?}"),
                );
              }
            },
            Calcit::Symbol { sym: s, .. } | Calcit::Str(s) => match record.index_of(s) {
              Some(pos) => {
                xs[v_idx].clone_into(&mut values[pos]);
              }
              None => {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!("&record:with unexpected field `{s}` for record: {def_fields:?}"),
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
          name: name.to_owned(),
          fields: def_fields.to_owned(),
          values: Arc::new(values),
          class: class.to_owned(),
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

pub fn get_class(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let args_size = xs.len();
  if args_size != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:class expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Record(CalcitRecord { class, .. }) => match class {
      Some(c) => Ok(Calcit::Record((**c).to_owned())),
      None => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&record:class expected a class, but received nil for {}", &xs[0]),
      ),
    },
    Calcit::Tuple(CalcitTuple { class, .. }) => match class {
      None => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&record:class expected a class, but received nil for {}", &xs[0]),
      ),
      Some(c) => Ok(Calcit::Record((**c).to_owned())),
    },
    a => {
      let msg = format!(
        "&record:class requires a record as prototype, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordClass).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn with_class(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let args_size = xs.len();
  if args_size < 2 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&record:with-class expected at least 2 arguments, but received:",
      xs,
    );
  }
  match (&xs[0], &xs[1]) {
    (
      Calcit::Record(CalcitRecord {
        name,
        fields: def_fields,
        values: v0,
        ..
      }),
      Calcit::Record(class),
    ) => Ok(Calcit::Record(CalcitRecord {
      name: name.to_owned(),
      fields: def_fields.to_owned(),
      values: v0.to_owned(),
      class: Some(Arc::new(class.to_owned())),
    })),
    (Calcit::Record { .. }, b) => {
      let msg = format!(
        "&record:with-class requires a record as class, but received: {}",
        type_of(&[b.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordWithClass).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (a, _b) => {
      let msg = format!(
        "&record:with-class requires a record, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordWithClass).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn record_from_map(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:from-map expected 2 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Record(record), Calcit::Map(ys)) => {
      let mut new_values = record.values.to_vec();
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
        match record.index_of(&key) {
          Some(idx) => new_values[idx] = v.to_owned(),
          None => {
            return CalcitErr::err_str(
              CalcitErrKind::Type,
              format!("&record:from-map invalid field {k} for record {:?}", record.fields),
            );
          }
        }
      }

      Ok(Calcit::Record(CalcitRecord {
        name: record.name.to_owned(),
        fields: record.fields.to_owned(),
        values: Arc::new(new_values),
        class: record.class.to_owned(),
      }))
    }
    (a, b) => {
      let msg = format!(
        "&record:from-map requires a record and a map, but received: {} and {}",
        type_of(&[a.to_owned()])?.lisp_str(),
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
    Calcit::Record(CalcitRecord { name, .. }) => Ok(Calcit::Tag(name.to_owned())),
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
pub fn turn_map(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:to-map expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Record(CalcitRecord { fields, values, .. }) => {
      let mut ys: rpds::HashTrieMapSync<Calcit, Calcit> = rpds::HashTrieMap::new_sync();
      for idx in 0..fields.len() {
        ys.insert_mut(Calcit::Tag(fields[idx].to_owned()), values[idx].to_owned());
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
  match (&xs[0], &xs[1]) {
    (
      Calcit::Record(CalcitRecord {
        name: left,
        fields: left_fields,
        ..
      }),
      Calcit::Record(CalcitRecord {
        name: right,
        fields: right_fields,
        ..
      }),
    ) => Ok(Calcit::Bool(left == right && left_fields == right_fields)),
    (a, b) => {
      let msg = format!(
        "&record:matches? requires 2 records, but received: {} and {}",
        type_of(&[a.to_owned()])?.lisp_str(),
        type_of(&[b.to_owned()])?.lisp_str()
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
    Calcit::Record(CalcitRecord { fields, .. }) => Ok(Calcit::Number(fields.len() as f64)),
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
      CalcitErr::err_nodes_with_hint(
        CalcitErrKind::Arity,
        "&record:get expected 2 arguments, but received:",
        xs,
        hint,
      )
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
      CalcitErr::err_nodes_with_hint(
        CalcitErrKind::Arity,
        "&record:get expected 2 arguments, but received:",
        xs,
        hint,
      )
    }
  }
}

pub fn assoc(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1), xs.get(2)) {
    (
      Some(Calcit::Record(
        record @ CalcitRecord {
          name,
          fields,
          values,
          class,
        },
      )),
      Some(a),
      Some(b),
    ) => match a {
      Calcit::Str(s) | Calcit::Symbol { sym: s, .. } => match record.index_of(s) {
        Some(pos) => {
          let mut new_values = (**values).to_owned();
          b.clone_into(&mut new_values[pos]);
          Ok(Calcit::Record(CalcitRecord {
            name: name.to_owned(),
            fields: fields.to_owned(),
            values: Arc::new(new_values),
            class: class.to_owned(),
          }))
        }
        None => CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("&record:assoc invalid field `{s}` for record: {fields:?}"),
        ),
      },
      Calcit::Tag(s) => match record.index_of(s.ref_str()) {
        Some(pos) => {
          let mut new_values = (**values).to_owned();
          b.clone_into(&mut new_values[pos]);
          Ok(Calcit::Record(CalcitRecord {
            name: name.to_owned(),
            fields: fields.to_owned(),
            values: Arc::new(new_values),
            class: class.to_owned(),
          }))
        }
        None => CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("&record:assoc invalid field `{s}` for record: {fields:?}"),
        ),
      },
      a => {
        let msg = format!(
          "&record:assoc requires a field in string/tag, but received: {} for record: {:?}",
          type_of(&[a.to_owned()])?.lisp_str(),
          fields
        );
        let hint = format_proc_examples_hint(&CalcitProc::NativeRecordAssoc).unwrap_or_default();
        CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
      }
    },
    (Some(_), None, _) | (Some(_), Some(_), None) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeRecordAssoc).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(
        CalcitErrKind::Arity,
        "&record:assoc expected 3 arguments, but received:",
        xs,
        hint,
      )
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
      CalcitErr::err_nodes_with_hint(
        CalcitErrKind::Arity,
        "&record:assoc expected 3 arguments, but received:",
        xs,
        hint,
      )
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
          record.fields
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
