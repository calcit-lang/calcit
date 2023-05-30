//! two kinds of atoms
//! - defined with `defatom`, which is global atom that retains after hot swapping
//! - defined with `atom`, which is barely a piece of local mutable state

use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

use cirru_edn::EdnTag;
use im_ternary_tree::TernaryTreeList;

use crate::primes::{Calcit, CalcitErr, CalcitItems, CalcitScope};
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
      Calcit::Fn {
        def_ns, scope, args, body, ..
      } => {
        let values = TernaryTreeList::from(&[v.to_owned(), prev.to_owned()]);
        runner::run_fn(&values, scope, args, body, def_ns.to_owned(), call_stack)?;
      }
      a => {
        return Err(CalcitErr::use_msg_stack_location(
          format!("expected fn to trigger after `reset!`, got {a}"),
          call_stack,
          a.get_location(),
        ))
      }
    }
  }
  Ok(())
}

/// syntax to prevent expr re-evaluating
pub fn defatom(expr: &CalcitItems, scope: &CalcitScope, file_ns: Arc<str>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  match (expr.get(0), expr.get(1)) {
    (Some(Calcit::Symbol { sym, ns, .. }), Some(code)) => {
      let mut path: String = (**ns).to_owned();
      path.push('/');
      path.push_str(sym);

      let path_info: Arc<str> = path.into();

      let defined = {
        let dict = REFS_DICT.lock().expect("read refs");
        dict.contains_key(&path_info)
      };

      if defined {
        let dict = REFS_DICT.lock().expect("read dict");
        let pair = dict.get(&path_info).expect("read pair");

        Ok(Calcit::Ref(path_info, pair.to_owned()))
      } else {
        let v = runner::evaluate_expr(code, scope, file_ns, call_stack)?;
        let pair_value = Arc::new(Mutex::new((v, HashMap::new())));
        let mut dict = REFS_DICT.lock().expect("read refs");
        dict.insert(path_info.to_owned(), pair_value.to_owned());
        Ok(Calcit::Ref(path_info, pair_value))
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
pub fn atom(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(value) => {
      let atom_idx = ATOM_ID_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      let path: String = format!("atom-{atom_idx}");

      let path_info: Arc<str> = path.into();

      let pair_value = Arc::new(Mutex::new((value.to_owned(), HashMap::new())));
      Ok(Calcit::Ref(path_info, pair_value))
    }
    _ => CalcitErr::err_str("atom expected 2 nodes"),
  }
}

pub fn deref(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Ref(_path, locked_pair)) => {
      let pair = (**locked_pair).lock().expect("read pair from block");
      Ok(pair.0.to_owned())
    }
    Some(a) => CalcitErr::err_str(format!("deref expected a ref, got: {a}")),
    _ => CalcitErr::err_str("deref expected 1 argument, got nothing"),
  }
}

/// need to be syntax since triggering internal functions requires program data
pub fn reset_bang(expr: &CalcitItems, scope: &CalcitScope, file_ns: Arc<str>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if expr.len() < 2 {
    return CalcitErr::err_str(format!("reset! excepted 2 arguments, got: {expr:?}"));
  }
  // println!("reset! {:?}", expr[0]);
  let target = runner::evaluate_expr(&expr[0], scope, file_ns.to_owned(), call_stack)?;
  let new_value = runner::evaluate_expr(&expr[1], scope, file_ns.to_owned(), call_stack)?;
  match (target, &new_value) {
    (Calcit::Ref(_path, locked_pair), v) => {
      modify_ref(locked_pair, v.to_owned(), call_stack)?;
      Ok(Calcit::Nil)
    }
    // if reset! called before deref, we need to trigger the thunk
    (Calcit::Thunk(code, _thunk_data), _) => match &expr[0] {
      Calcit::Symbol { sym, .. } => runner::evaluate_def_thunk(&code, &file_ns, sym, call_stack),
      _ => CalcitErr::err_str(format!("reset! expected a symbol, got: {:?}", expr[0])),
    },
    (a, b) => Err(CalcitErr::use_msg_stack_location(
      format!("reset! expected a ref and a value, got: {a} {b}"),
      call_stack,
      a.get_location().or_else(|| b.get_location()),
    )),
  }
}

pub fn add_watch(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Ref(_path, locked_pair)), Some(Calcit::Tag(k)), Some(f @ Calcit::Fn { .. })) => {
      let mut pair = locked_pair.lock().expect("trying to modify locked pair");
      if pair.1.contains_key(k) {
        CalcitErr::err_str(format!("add-watch failed, listener with key `{k}` existed"))
      } else {
        pair.1.insert(k.to_owned(), f.to_owned());
        Ok(Calcit::Nil)
      }
    }
    (Some(Calcit::Ref(..)), Some(Calcit::Tag(_)), Some(a)) => {
      CalcitErr::err_str(format!("add-watch expected fn instead of proc, got {a}"))
    }
    (Some(Calcit::Ref(..)), Some(a), Some(_)) => CalcitErr::err_str(format!("add-watch expected a tag, but got: {a}")),
    (Some(a), _, _) => CalcitErr::err_str(format!("add-watch expected ref, got: {a}")),
    (a, b, c) => CalcitErr::err_str(format!("add-watch expected ref, tag, function, got {a:?} {b:?} {c:?}")),
  }
}

pub fn remove_watch(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Ref(_path, locked_pair)), Some(Calcit::Tag(k))) => {
      let mut pair = locked_pair.lock().expect("trying to modify locked pair");
      if pair.1.contains_key(k) {
        pair.1.remove(k);
        Ok(Calcit::Nil)
      } else {
        CalcitErr::err_str(format!("remove-watch failed, listener with key `{k}` missing"))
      }
    }
    (Some(a), Some(b)) => CalcitErr::err_str(format!("remove-watch expected ref and tag, got: {a} {b}")),
    (a, b) => CalcitErr::err_str(format!("remove-watch expected 2 arguments, got {a:?} {b:?}")),
  }
}
