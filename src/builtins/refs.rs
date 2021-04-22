//! TODO watchers not implemented yet, after hot code swapping

use std::collections::HashMap;
use std::sync::Mutex;

use crate::primes::{Calcit, CalcitItems, CalcitScope};
use crate::program::ProgramCodeData;
use crate::runner;

lazy_static! {
  static ref REFS_DICT: Mutex<HashMap<String, Calcit>> = Mutex::new(HashMap::new());
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

      let dict = &mut REFS_DICT.lock().unwrap();

      if dict.contains_key(&path) {
        Ok(Calcit::Ref(path))
      } else {
        let v = runner::evaluate_expr(code, scope, file_ns, program_code)?;
        dict.insert(path.clone(), v);
        Ok(Calcit::Ref(path))
      }
    }
    (Some(a), Some(b)) => Err(format!("defref expected a symbol and an expression: {} , {}", a, b)),
    _ => Err(String::from("defref expected 2 nodes")),
  }
}

pub fn deref(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Ref(path)) => {
      let dict = &REFS_DICT.lock().unwrap();
      match dict.get(path) {
        Some(v) => Ok(v.clone()),
        None => Err(format!("found nothing after refer &{}", path)),
      }
    }
    Some(a) => Err(format!("deref expected a ref, got: {}", a)),
    _ => Err(String::from("deref expected 1 argument, got nothing")),
  }
}

pub fn reset_bang(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Ref(path)), Some(v)) => {
      let dict = &mut REFS_DICT.lock().unwrap();

      if dict.contains_key(path) {
        dict.insert(path.clone(), v.clone());
        Ok(Calcit::Nil)
      } else {
        Err(format!("missing pre-exisiting data for path &{}", path))
      }
    }
    (Some(a), Some(b)) => Err(format!("reset! expected a ref and a value, got: {} {}", a, b)),
    (a, b) => Err(format!("reset! expected 2 arguments, got: {:?} {:?}", a, b)),
  }
}

// TODO add-watch remove-watch
