//! TODO watchers not implemented yet, after hot code swapping

use std::collections::HashMap;
use std::sync::RwLock;

use crate::primes::{lookup_order_kwd_str, Calcit, CalcitErr, CalcitItems, CalcitScope};
use crate::program::ProgramCodeData;
use crate::runner;

type ValueAndListeners = (Calcit, HashMap<String, Calcit>);

lazy_static! {
  static ref REFS_DICT: RwLock<HashMap<String, ValueAndListeners>> = RwLock::new(HashMap::new());
}

// need functions with shorter lifetime to escape dead lock
fn read_ref(path: &str) -> Option<ValueAndListeners> {
  let dict = &REFS_DICT.read().unwrap();
  dict.get(path).map(|pair| pair.to_owned())
}

fn write_to_ref(path: String, v: Calcit, listeners: HashMap<String, Calcit>) {
  let mut dict = REFS_DICT.write().unwrap();
  let _ = (*dict).insert(path, (v, listeners));
}

fn modify_ref(path: String, v: Calcit, program_code: &ProgramCodeData) -> Result<(), CalcitErr> {
  let (prev, listeners) = read_ref(&path).unwrap();
  write_to_ref(path, v.to_owned(), listeners.to_owned());

  for f in listeners.values() {
    match f {
      Calcit::Fn(_, def_ns, _, def_scope, args, body) => {
        let values = im::vector![v.to_owned(), prev.to_owned()];
        runner::run_fn(&values, def_scope, args, body, def_ns, program_code)?;
      }
      a => return Err(CalcitErr::use_string(format!("expected fn to trigger after `reset!`, got {}", a))),
    }
  }
  Ok(())
}

/// syntax to prevent expr re-evaluating
pub fn defatom(expr: &CalcitItems, scope: &CalcitScope, file_ns: &str, program_code: &ProgramCodeData) -> Result<Calcit, CalcitErr> {
  match (expr.get(0), expr.get(1)) {
    (Some(Calcit::Symbol(s, ns, _def, _)), Some(code)) => {
      let mut path = ns.to_owned();
      path.push('/');
      path.push_str(s);

      if read_ref(&path).is_none() {
        let v = runner::evaluate_expr(code, scope, file_ns, program_code)?;
        write_to_ref(path.to_owned(), v, HashMap::new())
      }
      Ok(Calcit::Ref(path))
    }
    (Some(a), Some(b)) => Err(CalcitErr::use_string(format!(
      "defref expected a symbol and an expression: {} , {}",
      a, b
    ))),
    _ => Err(CalcitErr::use_str("defref expected 2 nodes")),
  }
}

pub fn deref(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Ref(path)) => match read_ref(path) {
      Some((v, _)) => Ok(v),
      None => Err(CalcitErr::use_string(format!("found nothing after refer &{}", path))),
    },
    Some(a) => Err(CalcitErr::use_string(format!("deref expected a ref, got: {}", a))),
    _ => Err(CalcitErr::use_str("deref expected 1 argument, got nothing")),
  }
}

/// need to be syntax since triggering internal functions requires program data
pub fn reset_bang(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, CalcitErr> {
  if expr.len() < 2 {
    return Err(CalcitErr::use_string(format!("reset! excepted 2 arguments, got: {:?}", expr)));
  }
  // println!("reset! {:?}", expr[0]);
  let target = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;
  let new_value = runner::evaluate_expr(&expr[1], scope, file_ns, program_code)?;
  match (target, new_value) {
    (Calcit::Ref(path), v) => {
      if read_ref(&path).is_none() {
        return Err(CalcitErr::use_string(format!("missing pre-exisiting data for path &{}", path)));
      }
      modify_ref(path, v, program_code)?;
      Ok(Calcit::Nil)
    }
    (a, b) => Err(CalcitErr::use_string(format!(
      "reset! expected a ref and a value, got: {} {}",
      a, b
    ))),
  }
}

pub fn add_watch(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Ref(path)), Some(Calcit::Keyword(k)), Some(Calcit::Fn(..))) => {
      let mut dict = REFS_DICT.write().unwrap();
      let (prev, listeners) = &(*dict).get(path).unwrap().to_owned();
      if listeners.contains_key(&lookup_order_kwd_str(k)) {
        Err(CalcitErr::use_string(format!(
          "add-watch failed, listener with key `{}` existed",
          k
        )))
      } else {
        let mut new_listeners = listeners.to_owned();
        new_listeners.insert(lookup_order_kwd_str(k), xs.get(2).unwrap().to_owned());
        let _ = (*dict).insert(path.to_owned(), (prev.to_owned(), new_listeners));
        Ok(Calcit::Nil)
      }
    }
    (Some(Calcit::Ref(_)), Some(Calcit::Keyword(_)), Some(a)) => {
      Err(CalcitErr::use_string(format!("add-watch expected fn instead of proc, got {}", a)))
    }
    (Some(Calcit::Ref(_)), Some(a), Some(_)) => Err(CalcitErr::use_string(format!("add-watch expected a keyword, but got: {}", a))),
    (Some(a), _, _) => Err(CalcitErr::use_string(format!("add-watch expected ref, got: {}", a))),
    (a, b, c) => Err(CalcitErr::use_string(format!(
      "add-watch expected ref, keyword, function, got {:?} {:?} {:?}",
      a, b, c
    ))),
  }
}

pub fn remove_watch(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Ref(path)), Some(Calcit::Keyword(k))) => {
      let mut dict = REFS_DICT.write().unwrap();
      let (prev, listeners) = &(*dict).get(path).unwrap().to_owned();
      if listeners.contains_key(&lookup_order_kwd_str(k)) {
        let mut new_listeners = listeners.to_owned();
        new_listeners.remove(&lookup_order_kwd_str(k));
        let _ = (*dict).insert(path.to_owned(), (prev.to_owned(), new_listeners));
        Ok(Calcit::Nil)
      } else {
        Err(CalcitErr::use_string(format!(
          "remove-watch failed, listener with key `{}` missing",
          k
        )))
      }
    }
    (Some(a), Some(b)) => Err(CalcitErr::use_string(format!(
      "remove-watch expected ref and keyword, got: {} {}",
      a, b
    ))),
    (a, b) => Err(CalcitErr::use_string(format!(
      "remove-watch expected 2 arguments, got {:?} {:?}",
      a, b
    ))),
  }
}
