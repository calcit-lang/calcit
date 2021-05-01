//! TODO watchers not implemented yet, after hot code swapping

use std::collections::HashMap;
use std::sync::Mutex;

use crate::primes::{Calcit, CalcitItems, CalcitScope};
use crate::program::ProgramCodeData;
use crate::runner;

type ValueAndListeners = (Calcit, HashMap<String, Calcit>);

lazy_static! {
  static ref REFS_DICT: Mutex<HashMap<String, ValueAndListeners>> = Mutex::new(HashMap::new());
}

// need functions with shorter lifetime to escape dead lock
fn read_ref(path: &str) -> Option<Calcit> {
  let dict = &REFS_DICT.lock().unwrap();
  match dict.get(path) {
    Some((v, _)) => Some(v.to_owned()),
    None => None,
  }
}

fn write_to_ref(path: String, v: Calcit) {
  let dict = &mut REFS_DICT.lock().unwrap();
  let _ = dict.insert(path, (v, HashMap::new()));
}

fn modify_ref(path: String, v: Calcit, program_code: &ProgramCodeData) -> Result<(), String> {
  let dict = &mut REFS_DICT.lock().unwrap();
  let (prev, listeners) = &dict.get(&path).unwrap().clone();
  let _ = dict.insert(path, (v.to_owned(), listeners.to_owned()));

  for f in listeners.values() {
    match f {
      Calcit::Fn(_, def_ns, _, def_scope, args, body) => {
        let values = im::vector![prev.to_owned(), v.to_owned()];
        runner::run_fn(&values, &def_scope, args, body, def_ns, program_code)?;
      }
      a => return Err(format!("expected fn to trigger after `reset!`, got {}", a)),
    }
  }
  Ok(())
}

/// syntax to prevent expr re-evaluating
pub fn defatom(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, String> {
  match (expr.get(0), expr.get(1)) {
    (Some(Calcit::Symbol(s, ns, _)), Some(code)) => {
      let mut path = ns.to_owned();
      path.push('/');
      path.push_str(s);

      if read_ref(&path).is_none() {
        let v = runner::evaluate_expr(code, scope, file_ns, program_code)?;
        write_to_ref(path.to_owned(), v)
      }
      Ok(Calcit::Ref(path))
    }
    (Some(a), Some(b)) => Err(format!("defref expected a symbol and an expression: {} , {}", a, b)),
    _ => Err(String::from("defref expected 2 nodes")),
  }
}

pub fn deref(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Ref(path)) => match read_ref(path) {
      Some(v) => Ok(v),
      None => Err(format!("found nothing after refer &{}", path)),
    },
    Some(a) => Err(format!("deref expected a ref, got: {}", a)),
    _ => Err(String::from("deref expected 1 argument, got nothing")),
  }
}

/// need to be syntax since triggering internal functions requires program data
pub fn reset_bang(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, String> {
  if expr.len() < 2 {
    return Err(format!("reset! excepted 2 arguments, got: {:?}", expr));
  }
  println!("reset! {:?}", expr[0]);
  let target = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;
  let new_value = runner::evaluate_expr(&expr[1], scope, file_ns, program_code)?;
  match (target, new_value) {
    (Calcit::Ref(path), v) => {
      if read_ref(&path).is_none() {
        return Err(format!("missing pre-exisiting data for path &{}", path));
      }
      modify_ref(path, v, program_code)?;
      Ok(Calcit::Nil)
    }
    (a, b) => Err(format!("reset! expected a ref and a value, got: {} {}", a, b)),
  }
}

pub fn add_watch(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Ref(path)), Some(Calcit::Keyword(k)), Some(Calcit::Fn(..))) => {
      let dict = &mut REFS_DICT.lock().unwrap();
      let (prev, listeners) = &dict.get(path).unwrap().clone();
      if listeners.contains_key(k) {
        Err(format!("add-watch failed, listener with key `{}` existed", k))
      } else {
        let mut new_listeners = listeners.clone();
        new_listeners.insert(k.to_owned(), xs.get(2).unwrap().to_owned());
        let _ = dict.insert(path.to_owned(), (prev.to_owned(), new_listeners));
        Ok(Calcit::Nil)
      }
    }
    (Some(Calcit::Ref(_)), Some(Calcit::Keyword(_)), Some(a)) => {
      Err(format!("add-watch expected fn instead of proc, got {}", a))
    }
    (Some(Calcit::Ref(_)), Some(a), Some(_)) => Err(format!("add-watch expected a keyword, but got: {}", a)),
    (Some(a), _, _) => Err(format!("add-watch expected ref, got: {}", a)),
    (a, b, c) => Err(format!(
      "add-watch expected ref, keyword, function, got {:?} {:?} {:?}",
      a, b, c
    )),
  }
}

pub fn remove_watch(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Ref(path)), Some(Calcit::Keyword(k))) => {
      let dict = &mut REFS_DICT.lock().unwrap();
      let (prev, listeners) = &dict.get(path).unwrap().clone();
      if listeners.contains_key(k) {
        let mut new_listeners = listeners.clone();
        new_listeners.remove(k);
        let _ = dict.insert(path.to_owned(), (prev.to_owned(), new_listeners));
        Ok(Calcit::Nil)
      } else {
        Err(format!("remove-watch failed, listener with key `{}` missing", k))
      }
    }
    (Some(a), Some(b)) => Err(format!("remove-watch expected ref and keyword, got: {} {}", a, b)),
    (a, b) => Err(format!("remove-watch expected 2 arguments, got {:?} {:?}", a, b)),
  }
}
