use crate::primes::finger_list::FingerList;
use crate::primes::{Calcit, CalcitErr, CalcitItems};
use std::sync::Arc;

pub fn new_set(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  let mut ys = rpds::HashTrieSet::new_sync();
  for x in xs {
    ys.insert_mut(x.to_owned());
  }
  Ok(Calcit::Set(ys))
}

pub fn call_include(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(xs)), Some(a)) => {
      let mut ys = xs.to_owned();
      ys.insert_mut(a.to_owned());
      Ok(Calcit::Set(ys))
    }
    (Some(a), _) => CalcitErr::err_str(format!("&include expect a set, but got: {}", a)),
    (a, b) => CalcitErr::err_str(format!("invalid arguments for &include: {:?} {:?}", a, b)),
  }
}

pub fn call_exclude(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(xs)), Some(a)) => {
      let mut ys = xs.to_owned();
      ys.remove_mut(a);
      Ok(Calcit::Set(ys))
    }
    (Some(a), _) => CalcitErr::err_str(format!("&exclude expect a set, but got: {}", a)),
    (a, b) => CalcitErr::err_str(format!("invalid arguments for &exclude: {:?} {:?}", a, b)),
  }
}
pub fn call_difference(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(a)), Some(Calcit::Set(b))) => {
      // rpds::HashTrieSetSync::difference has different semantics
      // https://docs.rs/im/12.2.0/im/struct.HashSet.html#method.difference
      let mut ys = a.to_owned();
      for item in b {
        ys.remove_mut(item);
      }
      Ok(Calcit::Set(ys))
    }
    (Some(a), Some(b)) => CalcitErr::err_str(format!("&difference expected 2 sets: {} {}", a, b)),
    (a, b) => CalcitErr::err_str(format!("&difference expected 2 arguments: {:?} {:?}", a, b)),
  }
}
pub fn call_union(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(a)), Some(Calcit::Set(b))) => {
      let mut c = a.to_owned();
      for x in b.iter() {
        c.insert_mut(x.to_owned());
      }
      Ok(Calcit::Set(c))
    }
    (Some(a), Some(b)) => CalcitErr::err_str(format!("&union expected 2 sets: {} {}", a, b)),
    (a, b) => CalcitErr::err_str(format!("&union expected 2 arguments: {:?} {:?}", a, b)),
  }
}
pub fn call_intersection(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(a)), Some(Calcit::Set(b))) => {
      let mut c: rpds::HashTrieSetSync<Calcit> = rpds::HashTrieSet::new_sync();
      for x in a.iter() {
        if b.contains(x) {
          c.insert_mut(x.to_owned())
        }
      }
      Ok(Calcit::Set(c))
    }
    (Some(a), Some(b)) => CalcitErr::err_str(format!("&set:intersection expected 2 sets: {} {}", a, b)),
    (a, b) => CalcitErr::err_str(format!("&set:intersection expected 2 arguments: {:?} {:?}", a, b)),
  }
}

/// turn hashset into list with a random order from internals
pub fn set_to_list(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Set(xs)) => {
      let mut ys: CalcitItems = FingerList::new_empty();
      for x in xs {
        ys = ys.push(x.to_owned());
      }
      Ok(Calcit::List(Arc::new(ys)))
    }
    Some(a) => CalcitErr::err_str(format!("&set:to-list expected a set: {}", a)),
    None => CalcitErr::err_str("&set:to-list expected 1 argument, got none"),
  }
}

pub fn count(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Set(ys)) => Ok(Calcit::Number(ys.size() as f64)),
    Some(a) => CalcitErr::err_str(format!("set count expected a set, got: {}", a)),
    None => CalcitErr::err_str("set count expected 1 argument"),
  }
}

pub fn empty_ques(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Set(ys)) => Ok(Calcit::Bool(ys.is_empty())),
    Some(a) => CalcitErr::err_str(format!("set empty? expected some set, got: {}", a)),
    None => CalcitErr::err_str("set empty? expected 1 argument"),
  }
}

pub fn includes_ques(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains(a))),
    (Some(a), ..) => CalcitErr::err_str(format!("sets `includes?` expected set, got: {}", a)),
    (None, ..) => CalcitErr::err_str(format!("sets `includes?` expected 2 arguments, got: {:?}", xs)),
  }
}

/// use builtin function since sets need to be handled specifically
pub fn first(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Set(ys)) => match ys.iter().next() {
      // TODO first element of a set.. need to be more sure...
      Some(v) => Ok(v.to_owned()),
      None => Ok(Calcit::Nil),
    },
    Some(a) => CalcitErr::err_str(format!("set:first expected a set, got: {}", a)),
    None => CalcitErr::err_str("set:first expected 1 argument"),
  }
}

pub fn rest(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Set(ys)) => match ys.iter().next() {
      Some(y0) => {
        let mut zs = ys.to_owned();
        zs.remove_mut(y0);
        Ok(Calcit::Set(zs))
      }
      None => Ok(Calcit::Nil),
    },
    Some(a) => CalcitErr::err_str(format!("set:rest expected a set, got: {}", a)),
    None => CalcitErr::err_str("set:rest expected 1 argument"),
  }
}
