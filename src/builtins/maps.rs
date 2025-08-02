use std::sync::Arc;

use crate::calcit::{Calcit, CalcitErr, CalcitErrKind, CalcitList, CalcitRecord};

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
    CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&{{}} expected an even number of arguments, but received: {}", CalcitList::from(xs)),
    )
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
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:dissoc expected a map, but received: {a}")),
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
    (Some(a), ..) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:get expected a map, but received: {a}")),
    (None, ..) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:get expected 2 arguments, but received:", xs),
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
            name,
            fields,
            values,
            class,
          },
        ),
        Calcit::Map(ys),
      ) => {
        let mut new_values = (**values).to_owned();
        for (k, v) in ys {
          match k {
            Calcit::Str(s) | Calcit::Symbol { sym: s, .. } => match record.index_of(s) {
              Some(pos) => v.clone_into(&mut new_values[pos]),
              None => return CalcitErr::err_str(CalcitErrKind::Type, format!("&map:merge invalid field `{s}` for record: {fields:?}")),
            },
            Calcit::Tag(s) => match record.index_of(s.ref_str()) {
              Some(pos) => v.clone_into(&mut new_values[pos]),
              None => return CalcitErr::err_str(CalcitErrKind::Type, format!("&map:merge invalid field `{s}` for record: {fields:?}")),
            },
            a => return CalcitErr::err_str(CalcitErrKind::Type, format!("&map:merge invalid field key, but received: {a}")),
          }
        }
        Ok(Calcit::Record(CalcitRecord {
          name: name.to_owned(),
          fields: fields.to_owned(),
          values: Arc::new(new_values),
          class: class.to_owned(),
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
    Some(Calcit::Record(CalcitRecord { fields, values, .. })) => {
      let mut zs: rpds::HashTrieSetSync<Calcit> = rpds::HashTrieSet::new_sync();
      for idx in 0..fields.len() {
        let chunk = vec![Calcit::Tag(fields[idx].to_owned()), values[idx].to_owned()];
        zs.insert_mut(Calcit::from(chunk));
      }
      Ok(Calcit::Set(zs))
    }
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:to-pairs expected a map, but received: {a}")),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "&map:to-pairs expected 1 argument, but received none"),
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
    (Some(a), Some(b)) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:merge-non-nil expected 2 maps, but received: {a} {b}")),
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
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:to-list expected a map, but received: {a}")),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "&map:to-list expected a map, but received none"),
  }
}

pub fn count(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Map(ys)) => Ok(Calcit::Number(ys.size() as f64)),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:count expected a map, but received: {a}")),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "&map:count expected 1 argument, but received none"),
  }
}

pub fn empty_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Map(ys)) => Ok(Calcit::Bool(ys.is_empty())),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:empty? expected a map, but received: {a}")),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "&map:empty? expected 1 argument, but received none"),
  }
}

pub fn contains_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains_key(a))),
    (Some(a), ..) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:contains? expected a map, but received: {a}")),
    (None, ..) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:contains? expected 2 arguments, but received:", xs),
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
        CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:assoc expected an odd number of arguments, but received:", xs)
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
    (Some(a), Some(b)) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:diff-keys expected 2 maps, but received: {a} {b}")),
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
    (Some(a), Some(b)) => CalcitErr::err_str(CalcitErrKind::Type, format!("&map:common-keys expected 2 maps, but received: {a} {b}")),
    (..) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&map:common-keys expected 2 arguments, but received:", xs),
  }
}
