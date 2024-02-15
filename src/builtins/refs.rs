//! two kinds of atoms
//! - defined with `defatom`, which is global atom that retains after hot swapping
//! - defined with `atom`, which is barely a piece of local mutable state

use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

use cirru_edn::EdnTag;
use im_ternary_tree::TernaryTreeList;

use crate::calcit::{Calcit, CalcitErr, CalcitImport, CalcitList, CalcitScope};
use crate::{call_stack::CallStackList, runner};

pub(crate) type ValueAndListeners = (Calcit, HashMap<EdnTag, Calcit>);

lazy_static! {
  static ref REFS_DICT: Mutex<HashMap<Arc<str>, Arc<Mutex<ValueAndListeners>>>> = Mutex::new(HashMap::new());
}

fn modify_ref(locked_pair: Arc<Mutex<ValueAndListeners>>, v: Calcit, call_stack: &CallStackList) -> Result<(), CalcitErr> {
  let (listeners, prev) = {
    let mut pair = locked_pair.lock().expect("read ref");
    let prev = pair.0.to_owned();
    if prev == v {
      // not need to modify
      return Ok(());
    }
    let listeners = pair.1.to_owned();
    pair.0 = v.to_owned();

    (listeners, prev)
  };

  for f in listeners.values() {
    match f {
      Calcit::Fn { info, .. } => {
        let values = TernaryTreeList::from(&[v.to_owned(), prev.to_owned()]);
        runner::run_fn(values, info, call_stack)?;
      }
      a => {
        return Err(CalcitErr::use_msg_stack_location(
          format!("expected fn to trigger after `reset!`, got: {a}"),
          call_stack,
          a.get_location(),
        ))
      }
    }
  }
  Ok(())
}

/// syntax to prevent expr re-evaluating
pub fn defatom(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  match (expr.get_inner(0), expr.get_inner(1)) {
    (Some(Calcit::Symbol { sym, info, .. }), Some(code)) => {
      let mut path: String = (*info.at_ns).to_owned();
      path.push('/');
      path.push_str(sym);

      let path_info: Arc<str> = path.into();

      // println!("defatom symbol {:?}", path_info);

      let defined = {
        let dict = REFS_DICT.lock().expect("read refs");
        dict.get(&path_info).map(ToOwned::to_owned)
        // need to release lock before calling `evaluate_expr`
      };

      match defined {
        Some(v) => Ok(Calcit::Ref(path_info, v.to_owned())),
        None => {
          let v = runner::evaluate_expr(code, scope, file_ns, call_stack)?;
          let pair_value = Arc::new(Mutex::new((v, HashMap::new())));
          let mut dict = REFS_DICT.lock().expect("read refs");
          dict.insert(path_info.to_owned(), pair_value.to_owned());
          Ok(Calcit::Ref(path_info, pair_value))
        }
      }
    }

    (Some(Calcit::Import(CalcitImport { def, ns, .. })), Some(code)) => {
      let mut path: String = ns.to_string();
      path.push('/');
      path.push_str(def);

      let path_info: Arc<str> = path.into();

      // println!("defatom import {:?}", path_info);

      let defined = {
        let dict = REFS_DICT.lock().expect("read refs");
        dict.get(&path_info).map(ToOwned::to_owned)
        // need to release lock before calling `evaluate_expr`
      };

      match defined {
        Some(v) => Ok(Calcit::Ref(path_info, v.to_owned())),
        None => {
          let v = runner::evaluate_expr(code, scope, file_ns, call_stack)?;
          let pair_value = Arc::new(Mutex::new((v, HashMap::new())));
          let mut dict = REFS_DICT.lock().expect("read refs");
          dict.insert(path_info.to_owned(), pair_value.to_owned());
          Ok(Calcit::Ref(path_info, pair_value))
        }
      }
    }
    (Some(a), Some(b)) => Err(CalcitErr::use_msg_stack_location(
      format!("defatom expected a symbol and an expression: {a} , {b}"),
      call_stack,
      a.get_location().or_else(|| b.get_location()),
    )),
    _ => Err(CalcitErr::use_msg_stack("defatom expected 2 nodes", call_stack)),
  }
}

/// dead simple counter for ID generator, better use nanoid in business
static ATOM_ID_GEN: AtomicUsize = AtomicUsize::new(0);

/// proc
pub fn atom(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(value) => {
      let atom_idx = ATOM_ID_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      let path: String = format!("atom-{atom_idx}");

      let path_info: Arc<str> = path.into();
      // println!("atom {:?}", path_info);

      let pair_value = Arc::new(Mutex::new((value.to_owned(), HashMap::new())));
      Ok(Calcit::Ref(path_info, pair_value))
    }
    _ => CalcitErr::err_str("atom expected 2 nodes"),
  }
}

/// previously `deref`, but `deref` now turned into a function calling `&atom:deref`
pub fn atom_deref(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Ref(_path, locked_pair)) => {
      // println!("deref import {:?}", _path);
      let pair = (**locked_pair).lock().expect("read pair from block");
      Ok(pair.0.to_owned())
    }
    Some(a) => CalcitErr::err_str(format!("deref expected a ref, got: {a}")),
    _ => CalcitErr::err_str("deref expected 1 argument, got nothing"),
  }
}

/// need to be syntax since triggering internal functions requires program data
pub fn reset_bang(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if expr.len() < 2 {
    return CalcitErr::err_nodes("reset! excepted 2 arguments, got:", &expr.into());
  }
  // println!("reset! {:?}", expr[0]);
  let target = runner::evaluate_expr(&expr[0], scope, file_ns, call_stack)?;
  let new_value = runner::evaluate_expr(&expr[1], scope, file_ns, call_stack)?;
  match (target, &new_value) {
    (Calcit::Ref(_path, locked_pair), v) => {
      // println!("reset defatom {:?} {}", _path, v);
      modify_ref(locked_pair, v.to_owned(), call_stack)?;
      Ok(Calcit::Nil)
    }
    // if reset! called before deref, we need to trigger the thunk
    (Calcit::Thunk(thunk), _) => match &expr[0] {
      Calcit::Symbol { .. } | Calcit::Import(CalcitImport { .. }) => {
        let ret = thunk.evaluated(scope, call_stack)?;
        match (ret, &new_value) {
          (Calcit::Ref(_path, locked_pair), v) => {
            // println!("reset defatom {:?} {}", _path, v);
            modify_ref(locked_pair, v.to_owned(), call_stack)?;
            Ok(Calcit::Nil)
          }
          (a, _) => Err(CalcitErr::use_msg_stack_location(
            format!("reset! expected a ref, got: {a}"),
            call_stack,
            a.get_location(),
          )),
        }
      }
      _ => CalcitErr::err_str(format!("reset! expected a symbol, got: {:?}", expr[0])),
    },
    (a, b) => Err(CalcitErr::use_msg_stack_location(
      format!("reset! expected a ref and a value, got: {a} {b}"),
      call_stack,
      a.get_location().or_else(|| b.get_location()),
    )),
  }
}

pub fn add_watch(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Ref(_path, locked_pair)), Some(Calcit::Tag(k)), Some(f @ Calcit::Fn { .. })) => {
      let mut pair = locked_pair.lock().expect("trying to modify locked pair");
      match pair.1.get(k) {
        Some(_) => CalcitErr::err_str(format!("add-watch failed, listener with key `{k}` existed")),
        None => {
          pair.1.insert(k.to_owned(), f.to_owned());
          Ok(Calcit::Nil)
        }
      }
    }
    (Some(Calcit::Ref(..)), Some(Calcit::Tag(_)), Some(a)) => {
      CalcitErr::err_str(format!("add-watch expected fn instead of proc, got: {a}"))
    }
    (Some(Calcit::Ref(..)), Some(a), Some(_)) => CalcitErr::err_str(format!("add-watch expected a tag, but got: {a}")),
    (Some(a), _, _) => CalcitErr::err_str(format!("add-watch expected ref, got: {a}")),
    (a, b, c) => CalcitErr::err_str(format!("add-watch expected ref, tag, function, got: {a:?} {b:?} {c:?}")),
  }
}

pub fn remove_watch(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Ref(_path, locked_pair)), Some(Calcit::Tag(k))) => {
      let mut pair = locked_pair.lock().expect("trying to modify locked pair");

      match pair.1.get(k) {
        None => CalcitErr::err_str(format!("remove-watch failed, listener with key `{k}` missing")),
        Some(_) => {
          pair.1.remove(k);
          Ok(Calcit::Nil)
        }
      }
    }
    (Some(a), Some(b)) => CalcitErr::err_str(format!("remove-watch expected ref and tag, got: {a} {b}")),
    (a, b) => CalcitErr::err_str(format!("remove-watch expected 2 arguments, got: {a:?} {b:?}")),
  }
}
