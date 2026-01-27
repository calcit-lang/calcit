use crate::builtins::meta::type_of;
use crate::calcit::{Calcit, CalcitErr, CalcitErrKind, CalcitList, CalcitProc, format_proc_examples_hint};

pub fn new_set(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let mut ys = rpds::HashTrieSet::new_sync();
  for x in xs {
    ys.insert_mut(x.to_owned());
  }
  Ok(Calcit::Set(ys))
}

pub fn call_include(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Set(xs)), Some(a)) => {
      let mut ys = xs.to_owned();
      ys.insert_mut(a.to_owned());
      Ok(Calcit::Set(ys))
    }
    (Some(_), None) => {
      let msg = format!(
        "&include requires 2 arguments, but received: {} arguments",
        if xs.is_empty() { 0 } else { 1 }
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeInclude).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Arity, msg, hint)
    }
    (Some(a), Some(_)) => {
      let msg = format!("&include requires a set, but received: {}", type_of(&[a.to_owned()])?.lisp_str());
      let hint = format_proc_examples_hint(&CalcitProc::NativeInclude).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (a, b) => {
      let msg = format!(
        "&include requires 2 arguments, but received: {} arguments",
        if a.is_none() && b.is_none() { 0 } else { 1 }
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeInclude).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Arity, msg, hint)
    }
  }
}

pub fn call_exclude(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Set(xs)), Some(a)) => {
      let mut ys = xs.to_owned();
      ys.remove_mut(a);
      Ok(Calcit::Set(ys))
    }
    (Some(_), None) => {
      let msg = format!(
        "&exclude expected 2 arguments, but received: {} arguments",
        if xs.is_empty() { 0 } else { 1 }
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeExclude).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Arity, msg, hint)
    }
    (Some(a), Some(_)) => {
      let msg = format!("&exclude requires a set, but received: {}", type_of(&[a.to_owned()])?.lisp_str());
      let hint = format_proc_examples_hint(&CalcitProc::NativeExclude).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&exclude expected 2 arguments, but received: {a:?} {b:?}"),
    ),
  }
}
pub fn call_difference(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Set(a)), Some(Calcit::Set(b))) => {
      // rpds::HashTrieSetSync::difference has different semantics
      // https://docs.rs/im/12.2.0/im/struct.HashSet.html#method.difference
      let mut ys = a.to_owned();
      for item in b {
        ys.remove_mut(item);
      }
      Ok(Calcit::Set(ys))
    }
    (Some(a), Some(b)) => {
      let msg = format!(
        "&difference requires 2 sets, but received: {} and {}",
        type_of(&[a.to_owned()])?.lisp_str(),
        type_of(&[b.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeDifference).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&difference expected 2 arguments, but received: {a:?} {b:?}"),
    ),
  }
}
pub fn call_union(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Set(a)), Some(Calcit::Set(b))) => {
      let mut c = a.to_owned();
      for x in b.iter() {
        c.insert_mut(x.to_owned());
      }
      Ok(Calcit::Set(c))
    }
    (Some(a), Some(b)) => {
      let msg = format!(
        "&union requires 2 sets, but received: {} and {}",
        type_of(&[a.to_owned()])?.lisp_str(),
        type_of(&[b.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeUnion).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&union expected 2 arguments, but received: {a:?} {b:?}"),
    ),
  }
}
pub fn call_intersection(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Set(a)), Some(Calcit::Set(b))) => {
      let mut c: rpds::HashTrieSetSync<Calcit> = rpds::HashTrieSet::new_sync();
      for x in a.iter() {
        if b.contains(x) {
          c.insert_mut(x.to_owned())
        }
      }
      Ok(Calcit::Set(c))
    }
    (Some(a), Some(b)) => {
      let msg = format!(
        "&set:intersection requires 2 sets, but received: {} and {}",
        type_of(&[a.to_owned()])?.lisp_str(),
        type_of(&[b.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeSetIntersection).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&set:intersection expected 2 arguments, but received: {a:?} {b:?}"),
    ),
  }
}

/// turn hashset into list with a random order from internals
pub fn set_to_list(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Set(xs)) => {
      let mut ys = vec![];
      for x in xs {
        ys.push(x.to_owned());
      }
      Ok(Calcit::from(ys))
    }
    Some(a) => {
      let msg = format!(
        "&set:to-list requires a set, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeSetToList).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => CalcitErr::err_str(CalcitErrKind::Arity, "&set:to-list expected 1 argument, but received none"),
  }
}

pub fn count(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Set(ys)) => Ok(Calcit::Number(ys.size() as f64)),
    Some(a) => {
      let msg = format!("&set:count requires a set, but received: {}", type_of(&[a.to_owned()])?.lisp_str());
      let hint = format_proc_examples_hint(&CalcitProc::NativeSetCount).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => CalcitErr::err_str(CalcitErrKind::Arity, "&set:count expected 1 argument, but received none"),
  }
}

pub fn empty_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Set(ys)) => Ok(Calcit::Bool(ys.is_empty())),
    Some(a) => {
      let msg = format!("&set:empty? requires a set, but received: {}", type_of(&[a.to_owned()])?.lisp_str());
      let hint = format_proc_examples_hint(&CalcitProc::NativeSetEmpty).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => CalcitErr::err_str(CalcitErrKind::Arity, "&set:empty? expected 1 argument, but received none"),
  }
}

pub fn includes_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Set(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains(a))),
    (Some(_), None) => {
      let hint = format_proc_examples_hint(&CalcitProc::NativeSetIncludes).unwrap_or_default();
      CalcitErr::err_nodes_with_hint(CalcitErrKind::Arity, "&set:includes? expected 2 arguments, but received:", xs, hint)
    }
    (Some(a), Some(_)) => {
      let msg = format!(
        "&set:includes? requires a set, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeSetIncludes).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (None, ..) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&set:includes? expected 2 arguments, but received:", xs),
  }
}

/// use builtin function since sets need to be handled specifically
pub fn destruct(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Set(ys)) => match ys.iter().next() {
      // first element of a set might be random
      Some(y0) => {
        let mut zs = ys.to_owned();
        zs.remove_mut(y0);
        Ok(Calcit::from(CalcitList::from(&[y0.to_owned(), Calcit::Set(zs)])))
      }
      None => Ok(Calcit::Nil),
    },
    Some(a) => {
      let msg = format!(
        "&set:destruct requires a set, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeSetDestruct).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => CalcitErr::err_str(CalcitErrKind::Arity, "&set:destruct expected 1 argument, but received none"),
  }
}
