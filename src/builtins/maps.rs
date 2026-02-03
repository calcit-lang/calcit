use std::sync::Arc;

use crate::builtins::meta::type_of;
use crate::calcit::{Calcit, CalcitErr, CalcitErrKind, CalcitList, CalcitProc, CalcitRecord, format_proc_examples_hint};

use crate::util::number::is_even;

pub fn call_new_map(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if is_even(xs.len()) {
    let n = xs.len() >> 1;
    let mut ys = rpds::HashTrieMap::new_sync();
    for i in 0..n {
      ys.insert_mut(xs[i << 1].to_owned(), xs[(i << 1) + 1].to_owned());
    }
    Ok(Calcit::Map(ys))
  } else {
    let msg = format!(
      "&{{}} requires an even number of arguments (key-value pairs), but received: {} arguments",
      xs.len()
    );
    let hint = format_proc_examples_hint(&CalcitProc::NativeMap).unwrap_or_default();
    CalcitErr::err_str_with_hint(CalcitErrKind::Arity, msg, hint)
  }
}

pub fn dissoc(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:dissoc expected at least 2 arguments, but received:", xs);
  }
  match xs.first() {
    Some(Calcit::Map(base)) => {
      let ys = &mut base.to_owned();
      let mut skip_first = true;
      for x in xs {
        if skip_first {
          skip_first = false;
          continue;
        }
        ys.remove_mut(x);
      }
      Ok(Calcit::Map(ys.to_owned()))
    }
    Some(a) => {
      let msg = format!("&map:dissoc requires a map, but received: {}", type_of(&[a.to_owned()])?.lisp_str());
      let hint = format_proc_examples_hint(&CalcitProc::NativeMapDissoc).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    _ => CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:dissoc expected 2 arguments, but received:", xs),
  }
}

pub fn get(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(a)) => {
      let ys = &mut xs.to_owned();
      match ys.get(a) {
        Some(v) => Ok(v.to_owned()),
        None => Ok(Calcit::Nil),
      }
    }
    (Some(_), None) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeMapGet).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(
        CalcitErrKind::Arity,
        "&map:get requires 2 arguments (map and key), but received:",
        xs,
        hint,
      )
    }
    (Some(a), Some(_)) => {
      let msg = format!("&map:get requires a map, but received: {}", type_of(&[a.to_owned()])?.lisp_str());
      let hint = format_proc_examples_hint(&CalcitProc::NativeMapGet).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (None, ..) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeMapGet).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(
        CalcitErrKind::Arity,
        "&map:get requires 2 arguments (map and key), but received:",
        xs,
        hint,
      )
    }
  }
}

pub fn call_merge(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() == 2 {
    match (&xs[0], &xs[1]) {
      (Calcit::Map(xs), Calcit::Nil) => Ok(Calcit::Map(xs.to_owned())),
      (Calcit::Map(xs), Calcit::Map(ys)) => {
        let mut zs: rpds::HashTrieMapSync<Calcit, Calcit> = xs.to_owned();
        for (k, v) in ys {
          zs.insert_mut(k.to_owned(), v.to_owned());
        }
        Ok(Calcit::Map(zs))
      }
      (
        Calcit::Record(
          record @ CalcitRecord {
            struct_ref,
            values,
            classes,
          },
        ),
        Calcit::Map(ys),
      ) => {
        let mut new_values = (**values).to_owned();
        for (k, v) in ys {
          match k {
            Calcit::Str(s) | Calcit::Symbol { sym: s, .. } => match record.index_of(s) {
              Some(pos) => v.clone_into(&mut new_values[pos]),
              None => {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!("&map:merge invalid field `{s}` for record: {:?}", struct_ref.fields),
                );
              }
            },
            Calcit::Tag(s) => match record.index_of(s.ref_str()) {
              Some(pos) => v.clone_into(&mut new_values[pos]),
              None => {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!("&map:merge invalid field `{s}` for record: {:?}", struct_ref.fields),
                );
              }
            },
            a => return CalcitErr::err_str(CalcitErrKind::Type, format!("&map:merge invalid field key, but received: {a}")),
          }
        }
        Ok(Calcit::Record(CalcitRecord {
          struct_ref: record.struct_ref.to_owned(),
          values: Arc::new(new_values),
          classes: classes.clone(),
        }))
      }
      (a, b) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:merge expected 2 maps, but received: {a} {b}")),
    }
  } else {
    CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:merge expected 2 arguments, but received:", xs)
  }
}

/// to set
pub fn to_pairs(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    // get a random order from internals
    Some(Calcit::Map(ys)) => {
      let mut zs: rpds::HashTrieSetSync<Calcit> = rpds::HashTrieSet::new_sync();
      for (k, v) in ys {
        let chunk = vec![k.to_owned(), v.to_owned()];
        zs.insert_mut(Calcit::from(chunk));
      }
      Ok(Calcit::Set(zs))
    }
    Some(Calcit::Record(CalcitRecord { struct_ref, values, .. })) => {
      let mut zs: rpds::HashTrieSetSync<Calcit> = rpds::HashTrieSet::new_sync();
      for idx in 0..struct_ref.fields.len() {
        let chunk = vec![Calcit::Tag(struct_ref.fields[idx].to_owned()), values[idx].to_owned()];
        zs.insert_mut(Calcit::from(chunk));
      }
      Ok(Calcit::Set(zs))
    }
    Some(a) => {
      let msg = format!(
        "&map:to-pairs requires a map or record, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::ToPairs).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => {
      let hint = format_proc_examples_hint(&CalcitProc::ToPairs).unwrap_or_default();
      CalcitErr::err_str_with_hint(
        CalcitErrKind::Arity,
        "&map:to-pairs requires 1 argument, but received none".to_string(),
        hint,
      )
    }
  }
}

pub fn call_merge_non_nil(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(Calcit::Map(ys))) => {
      let mut zs: rpds::HashTrieMapSync<Calcit, Calcit> = xs.to_owned();
      for (k, v) in ys {
        if *v != Calcit::Nil {
          zs.insert_mut(k.to_owned(), v.to_owned());
        }
      }
      Ok(Calcit::Map(zs))
    }
    (Some(a), Some(b)) => {
      let msg = format!(
        "&map:merge-non-nil requires 2 maps, but received: {} and {}",
        type_of(&[a.to_owned()])?.lisp_str(),
        type_of(&[b.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeMergeNonNil).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (_, _) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:merge-non-nil expected 2 arguments, but received:", xs),
  }
}

/// out to list, but with a arbitrary order
pub fn to_list(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Map(m)) => {
      let mut ys = vec![];
      for (k, v) in m {
        let zs = vec![k.to_owned(), v.to_owned()];
        ys.push(Calcit::from(zs));
      }
      Ok(Calcit::from(ys))
    }
    Some(a) => {
      let msg = format!(
        "&map:to-list requires a map, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeMapToList).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeMapToList).unwrap_or_default();
      CalcitErr::err_str_with_hint(
        CalcitErrKind::Arity,
        "&map:to-list requires 1 argument, but received none".to_string(),
        hint,
      )
    }
  }
}

pub fn count(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Map(ys)) => Ok(Calcit::Number(ys.size() as f64)),
    Some(a) => {
      let msg = format!("&map:count requires a map, but received: {}", type_of(&[a.to_owned()])?.lisp_str());
      let hint = format_proc_examples_hint(&CalcitProc::NativeMapCount).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeMapCount).unwrap_or_default();
      CalcitErr::err_str_with_hint(
        CalcitErrKind::Arity,
        "&map:count requires 1 argument, but received none".to_string(),
        hint,
      )
    }
  }
}

pub fn empty_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Map(ys)) => Ok(Calcit::Bool(ys.is_empty())),
    Some(a) => {
      let msg = format!("&map:empty? requires a map, but received: {}", type_of(&[a.to_owned()])?.lisp_str());
      let hint = format_proc_examples_hint(&CalcitProc::NativeMapEmpty).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeMapEmpty).unwrap_or_default();
      CalcitErr::err_str_with_hint(
        CalcitErrKind::Arity,
        "&map:empty? requires 1 argument, but received none".to_string(),
        hint,
      )
    }
  }
}

pub fn contains_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains_key(a))),
    (Some(_), None) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeMapContains).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(
        CalcitErrKind::Arity,
        "&map:contains? requires 2 arguments (map and key), but received:",
        xs,
        hint,
      )
    }
    (Some(a), Some(_)) => {
      let msg = format!(
        "&map:contains? requires a map, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeMapContains).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (None, ..) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeMapContains).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(
        CalcitErrKind::Arity,
        "&map:contains? requires 2 arguments (map and key), but received:",
        xs,
        hint,
      )
    }
  }
}

pub fn includes_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Map(ys)), Some(a)) => {
      for (_k, v) in ys {
        if v == a {
          return Ok(Calcit::Bool(true));
        }
      }
      Ok(Calcit::Bool(false))
    }
    (Some(a), ..) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:includes? expected a map, but received: {a}")),
    (None, ..) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:includes? expected 2 arguments, but received:", xs),
  }
}

pub fn destruct(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Map(ys)) => match ys.keys().next() {
      // order not stable
      Some(k0) => {
        let mut zs = ys.to_owned();
        zs.remove_mut(k0);
        Ok(Calcit::from(CalcitList::from(&[k0.to_owned(), ys[k0].to_owned(), Calcit::Map(zs)])))
      }
      None => Ok(Calcit::Nil),
    },
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:destruct expected a map, but received: {a}")),
    None => CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:destruct expected 1 argument, but received:", xs),
  }
}

pub fn assoc(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Map(base)) => {
      if xs.len() % 2 != 1 {
        CalcitErr::err_nodes(
          CalcitErrKind::Arity,
          "&map:assoc expected an odd number of arguments, but received:",
          xs,
        )
      } else {
        let size = (xs.len() - 1) / 2;
        let mut ys = base.to_owned();
        for idx in 0..size {
          ys.insert_mut(xs[idx * 2 + 1].to_owned(), xs[idx * 2 + 2].to_owned());
        }
        Ok(Calcit::Map(ys))
      }
    }
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:assoc expected a map, but received: {a}")),
    None => CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:assoc expected 3 arguments, but received:", xs),
  }
}

pub fn diff_new(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(Calcit::Map(ys))) => {
      let zs = &mut xs.to_owned();
      for k in ys.keys() {
        if zs.contains_key(k) {
          zs.remove_mut(k);
        }
      }
      Ok(Calcit::Map(zs.to_owned()))
    }
    (Some(a), Some(b)) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:diff-new expected 2 maps, but received: {a} {b}")),
    (..) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:diff-new expected 2 arguments, but received:", xs),
  }
}

pub fn diff_keys(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(Calcit::Map(ys))) => {
      let mut ks: rpds::HashTrieSetSync<Calcit> = rpds::HashTrieSet::new_sync();
      for k in xs.keys() {
        if !ys.contains_key(k) {
          ks.insert_mut(k.to_owned());
        }
      }
      Ok(Calcit::Set(ks))
    }
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&map:diff-keys expected 2 maps, but received: {a} {b}"),
    ),
    (..) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:diff-keys expected 2 arguments, but received:", xs),
  }
}

pub fn common_keys(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(Calcit::Map(ys))) => {
      let mut ks: rpds::HashTrieSetSync<Calcit> = rpds::HashTrieSet::new_sync();
      for k in xs.keys() {
        if ys.contains_key(k) {
          ks.insert_mut(k.to_owned());
        }
      }
      Ok(Calcit::Set(ks))
    }
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&map:common-keys expected 2 maps, but received: {a} {b}"),
    ),
    (..) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:common-keys expected 2 arguments, but received:", xs),
  }
}
