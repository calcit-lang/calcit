//! TODO watchers not implemented yet, after hot code swapping

use std::collections::HashMap;
use std::sync::Mutex;

use crate::primes::{Calcit, CalcitItems, CalcitScope};
use crate::program::ProgramCodeData;
use crate::runner;

lazy_static! {
  static ref REFS_DICT: Mutex<HashMap<String, Calcit>> = Mutex::new(HashMap::new());
}

// need functions with shorter lifetime to escape dead lock
fn read_ref(path: &str) -> Option<Calcit> {
  let dict = &REFS_DICT.lock().unwrap();
  match dict.get(path) {
    Some(v) => Some(v.to_owned()),
    None => None,
  }
}

fn write_to_ref(path: String, v: Calcit) {
  let dict = &mut REFS_DICT.lock().unwrap();
  let _ = dict.insert(path, v);
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
      let mut path = ns.to_string();
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

pub fn reset_bang(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Ref(path)), Some(v)) => {
      if read_ref(path).is_none() {
        return Err(format!("missing pre-exisiting data for path &{}", path));
      }
      write_to_ref(path.to_owned(), v.to_owned());
      Ok(Calcit::Nil)
    }
    (Some(a), Some(b)) => Err(format!("reset! expected a ref and a value, got: {} {}", a, b)),
    (a, b) => Err(format!("reset! expected 2 arguments, got: {:?} {:?}", a, b)),
  }
}

// TODO
pub fn add_watch(_xs: &CalcitItems) -> Result<Calcit, String> {
  Ok(Calcit::Nil)
}
// TODO
pub fn remove_watch(_xs: &CalcitItems) -> Result<Calcit, String> {
  Ok(Calcit::Nil)
}
