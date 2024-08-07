use std::sync::Arc;

use crate::calcit::{Calcit, CalcitErr, CalcitList, CalcitRecord};

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
    CalcitErr::err_str(format!("&{{}} expected even number of arguments, got: {}", CalcitList::from(xs)))
  }
}

pub fn dissoc(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    return CalcitErr::err_nodes("map dissoc expected at least 2 arguments:", xs);
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
    Some(a) => CalcitErr::err_str(format!("map dissoc expected a map, got: {a}")),
    _ => CalcitErr::err_nodes("map dissoc expected 2 arguments, got:", xs),
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
    (Some(a), ..) => CalcitErr::err_str(format!("map &get expected map, got: {a}")),
    (None, ..) => CalcitErr::err_nodes("map &get expected 2 arguments, got:", xs),
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
              None => return CalcitErr::err_str(format!("invalid field `{s}` for {fields:?}")),
            },
            Calcit::Tag(s) => match record.index_of(s.ref_str()) {
              Some(pos) => v.clone_into(&mut new_values[pos]),
              None => return CalcitErr::err_str(format!("invalid field `{s}` for {fields:?}")),
            },
            a => return CalcitErr::err_str(format!("invalid field key: {a}")),
          }
        }
        Ok(Calcit::Record(CalcitRecord {
          name: name.to_owned(),
          fields: fields.to_owned(),
          values: Arc::new(new_values),
          class: class.to_owned(),
        }))
      }
      (a, b) => CalcitErr::err_str(format!("expected 2 maps, got: {a} {b}")),
    }
  } else {
    CalcitErr::err_nodes("expected 2 arguments, got:", xs)
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
    Some(a) => CalcitErr::err_str(format!("to-pairs expected a map, got: {a}")),
    None => CalcitErr::err_str("to-pairs expected 1 argument, got nothing"),
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
    (Some(a), Some(b)) => CalcitErr::err_str(format!("expected 2 maps, got: {a} {b}")),
    (_, _) => CalcitErr::err_nodes("expected 2 arguments, got:", xs),
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
    Some(a) => CalcitErr::err_str(format!("&map:to-list expected a map, got: {a}")),
    None => CalcitErr::err_str("&map:to-list expected a map, got nothing"),
  }
}

pub fn count(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Map(ys)) => Ok(Calcit::Number(ys.size() as f64)),
    Some(a) => CalcitErr::err_str(format!("map count expected a map, got: {a}")),
    None => CalcitErr::err_str("map count expected 1 argument"),
  }
}

pub fn empty_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Map(ys)) => Ok(Calcit::Bool(ys.is_empty())),
    Some(a) => CalcitErr::err_str(format!("map empty? expected some map, got: {a}")),
    None => CalcitErr::err_str("map empty? expected 1 argument"),
  }
}

pub fn contains_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains_key(a))),
    (Some(a), ..) => CalcitErr::err_str(format!("map contains? expected a map, got: {a}")),
    (None, ..) => CalcitErr::err_nodes("map contains? expected 2 arguments, got:", xs),
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
    (Some(a), ..) => CalcitErr::err_str(format!("map `includes?` expected a map, got: {a}")),
    (None, ..) => CalcitErr::err_nodes("map `includes?` expected 2 arguments, got:", xs),
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
    Some(a) => CalcitErr::err_str(format!("&map:destruct expected a map, got: {a}")),
    None => CalcitErr::err_nodes("&map:destruct expected 1 argument, got:", xs),
  }
}

pub fn assoc(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Map(base)) => {
      if xs.len() % 2 != 1 {
        CalcitErr::err_nodes("map:assoc expected odd number of arguments, got:", xs)
      } else {
        let size = (xs.len() - 1) / 2;
        let mut ys = base.to_owned();
        for idx in 0..size {
          ys.insert_mut(xs[idx * 2 + 1].to_owned(), xs[idx * 2 + 2].to_owned());
        }
        Ok(Calcit::Map(ys))
      }
    }
    Some(a) => CalcitErr::err_str(format!("map:assoc expected a map, got: {a}")),
    None => CalcitErr::err_nodes("map:assoc expected 3 arguments, got:", xs),
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
    (Some(a), Some(b)) => CalcitErr::err_str(format!("map:new_entries expected 2 maps, got: {a} {b}")),
    (..) => CalcitErr::err_nodes("map:diff-new expected 2 arguments, got", xs),
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
    (Some(a), Some(b)) => CalcitErr::err_str(format!("map:diff-keys expected 2 maps, got: {a} {b}")),
    (..) => CalcitErr::err_nodes("map:diff-keys expected 2 arguments, got:", xs),
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
    (Some(a), Some(b)) => CalcitErr::err_str(format!("map:diff-keys expected 2 maps, got: {a} {b}")),
    (..) => CalcitErr::err_nodes("map:common-keys expected 2 arguments, got:", xs),
  }
}
