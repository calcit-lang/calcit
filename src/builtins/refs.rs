//! TODO watchers not implemented yet, after hot code swapping

use std::collections::HashMap;
use std::sync::RwLock;

use cirru_edn::EdnKwd;

use crate::primes::{Calcit, CalcitErr, CalcitItems, CalcitScope};
use crate::{call_stack::CallStackVec, runner};

type ValueAndListeners = (Calcit, HashMap<EdnKwd, Calcit>);

lazy_static! {
  static ref REFS_DICT: RwLock<HashMap<Box<str>, ValueAndListeners>> = RwLock::new(HashMap::new());
}

// need functions with shorter lifetime to escape dead lock
fn read_ref(path: &str) -> Option<ValueAndListeners> {
  let dict = &REFS_DICT.read().unwrap();
  dict.get(path).map(|pair| pair.to_owned())
}

fn write_to_ref(path: &Box<str>, v: Calcit, listeners: HashMap<EdnKwd, Calcit>) {
  let mut dict = REFS_DICT.write().unwrap();
  let _ = (*dict).insert(path.to_owned(), (v, listeners));
}

fn modify_ref(path: &Box<str>, v: Calcit, call_stack: &CallStackVec) -> Result<(), CalcitErr> {
  let (prev, listeners) = read_ref(&path).unwrap();
  write_to_ref(path, v.to_owned(), listeners.to_owned());

  for f in listeners.values() {
    match f {
      Calcit::Fn(_, def_ns, _, def_scope, args, body) => {
        let values = rpds::vector_sync![v.to_owned(), prev.to_owned()];
        runner::run_fn(&values, def_scope, args, body, def_ns, call_stack)?;
      }
      a => {
        return Err(CalcitErr::use_msg_stack(
          format!("expected fn to trigger after `reset!`, got {}", a),
          call_stack,
        ))
      }
    }
  }
  Ok(())
}

/// syntax to prevent expr re-evaluating
pub fn defatom(expr: &CalcitItems, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackVec) -> Result<Calcit, CalcitErr> {
  match (expr.get(0), expr.get(1)) {
    (Some(Calcit::Symbol(s, ns, _def, _)), Some(code)) => {
      let mut path = (**ns).to_owned();
      path.push('/');
      path.push_str(s);

      if read_ref(&path).is_none() {
        let v = runner::evaluate_expr(code, scope, file_ns, call_stack)?;
        write_to_ref(&path.to_owned().into_boxed_str(), v, HashMap::new())
      }
      Ok(Calcit::Ref(path.into_boxed_str()))
    }
    (Some(a), Some(b)) => Err(CalcitErr::use_msg_stack(
      format!("defref expected a symbol and an expression: {} , {}", a, b),
      call_stack,
    )),
    _ => Err(CalcitErr::use_msg_stack("defref expected 2 nodes", call_stack)),
  }
}

pub fn deref(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Ref(path)) => match read_ref(path) {
      Some((v, _)) => Ok(v),
      None => CalcitErr::err_str(format!("found nothing after refer &{}", path)),
    },
    Some(a) => CalcitErr::err_str(format!("deref expected a ref, got: {}", a)),
    _ => CalcitErr::err_str("deref expected 1 argument, got nothing"),
  }
}

/// need to be syntax since triggering internal functions requires program data
pub fn reset_bang(expr: &CalcitItems, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackVec) -> Result<Calcit, CalcitErr> {
  if expr.len() < 2 {
    return CalcitErr::err_str(format!("reset! excepted 2 arguments, got: {:?}", expr));
  }
  // println!("reset! {:?}", expr[0]);
  let target = runner::evaluate_expr(&expr[0], scope, file_ns, call_stack)?;
  let new_value = runner::evaluate_expr(&expr[1], scope, file_ns, call_stack)?;
  match (target, new_value) {
    (Calcit::Ref(path), v) => {
      if read_ref(&path).is_none() {
        return CalcitErr::err_str(format!("missing pre-exisiting data for path &{}", path));
      }
      modify_ref(&path, v, call_stack)?;
      Ok(Calcit::Nil)
    }
    (a, b) => Err(CalcitErr::use_msg_stack(
      format!("reset! expected a ref and a value, got: {} {}", a, b),
      call_stack,
    )),
  }
}

pub fn add_watch(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Ref(path)), Some(Calcit::Keyword(k)), Some(Calcit::Fn(..))) => {
      let mut dict = REFS_DICT.write().unwrap();
      let (prev, listeners) = &(*dict).get(path).unwrap().to_owned();
      if listeners.contains_key(k) {
        CalcitErr::err_str(format!("add-watch failed, listener with key `{}` existed", k))
      } else {
        let mut new_listeners = listeners.to_owned();
        new_listeners.insert(k.to_owned(), xs.get(2).unwrap().to_owned());
        let _ = (*dict).insert(path.to_owned(), (prev.to_owned(), new_listeners));
        Ok(Calcit::Nil)
      }
    }
    (Some(Calcit::Ref(_)), Some(Calcit::Keyword(_)), Some(a)) => {
      CalcitErr::err_str(format!("add-watch expected fn instead of proc, got {}", a))
    }
    (Some(Calcit::Ref(_)), Some(a), Some(_)) => CalcitErr::err_str(format!("add-watch expected a keyword, but got: {}", a)),
    (Some(a), _, _) => CalcitErr::err_str(format!("add-watch expected ref, got: {}", a)),
    (a, b, c) => CalcitErr::err_str(format!("add-watch expected ref, keyword, function, got {:?} {:?} {:?}", a, b, c)),
  }
}

pub fn remove_watch(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Ref(path)), Some(Calcit::Keyword(k))) => {
      let mut dict = REFS_DICT.write().unwrap();
      let (prev, listeners) = &(*dict).get(path).unwrap().to_owned();
      if listeners.contains_key(k) {
        let mut new_listeners = listeners.to_owned();
        new_listeners.remove(k);
        let _ = (*dict).insert(path.to_owned(), (prev.to_owned(), new_listeners));
        Ok(Calcit::Nil)
      } else {
        CalcitErr::err_str(format!("remove-watch failed, listener with key `{}` missing", k))
      }
    }
    (Some(a), Some(b)) => CalcitErr::err_str(format!("remove-watch expected ref and keyword, got: {} {}", a, b)),
    (a, b) => CalcitErr::err_str(format!("remove-watch expected 2 arguments, got {:?} {:?}", a, b)),
  }
}
