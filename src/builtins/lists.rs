use core::cmp::Ordering;
use std::sync::Arc;

use im_ternary_tree::TernaryTreeList;

use crate::calcit::{Calcit, CalcitErr, CalcitList, CalcitTuple};
use crate::util::number::f64_to_usize;

use crate::builtins;
use crate::call_stack::CallStackList;
use crate::runner;

pub fn new_list(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  Ok(Calcit::List(Arc::new(xs.into())))
}

pub fn count(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes("list count expected a list, got:", &xs);
  }
  match &xs[0] {
    Calcit::List(ys) => Ok(Calcit::Number(ys.len() as f64)),
    a => CalcitErr::err_str(format!("list count expected a list, got: {a}")),
  }
}

pub fn nth(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_str(format!("nth expected 2 argument, got: {}", CalcitList::from(xs)));
  }
  match (&xs[0], &xs[1]) {
    (Calcit::List(ys), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => match ys.get(idx) {
        Some(v) => Ok((*v).to_owned()),
        None => Ok(Calcit::Nil),
      },
      Err(e) => CalcitErr::err_str(format!("nth expect usize, {e}")),
    },
    (_, _) => CalcitErr::err_str(format!("nth expected a list and an index, got: {}", CalcitList::from(xs))),
  }
}

pub fn slice(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 && xs.len() != 3 {
    return CalcitErr::err_str(format!("slice expected 2~3 argument, got: {}", CalcitList::from(xs)));
  }
  match (&xs[0], &xs[1]) {
    (Calcit::List(ys), Calcit::Number(from)) => {
      let to_idx = match xs.get(2) {
        Some(Calcit::Number(to)) => {
          let idx: usize = unsafe { to.to_int_unchecked() };
          idx
        }
        Some(a) => return CalcitErr::err_str(format!("slice expected number index, got: {a}")),
        None => ys.len(),
      };
      let from_idx: usize = unsafe { from.to_int_unchecked() };

      Ok(Calcit::List(Arc::new(ys.slice(from_idx, to_idx)?)))
    }
    (a, b) => CalcitErr::err_str(format!("slice expected list and indexes: {a} {b}")),
  }
}

pub fn append(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_str(format!("append expected 2 arguments, got: {}", CalcitList::from(xs)));
  }
  match &xs[0] {
    Calcit::List(ys) => Ok(Calcit::List(Arc::new(ys.push_right(xs[1].to_owned())))),
    a => CalcitErr::err_str(format!("append expected a list: {a}")),
  }
}

pub fn prepend(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(ys)), Some(a)) => Ok(Calcit::List(Arc::new(ys.push_left(a.to_owned())))),
    (Some(a), _) => CalcitErr::err_str(format!("prepend expected list, got: {a}")),
    (None, _) => CalcitErr::err_str("prepend expected 2 arguments, got nothing"),
  }
}

pub fn rest(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes("list:rest expected a list, got:", &xs);
  }
  match &xs[0] {
    Calcit::List(ys) => {
      if ys.is_empty() {
        Ok(Calcit::Nil)
      } else {
        Ok(Calcit::List(Arc::new(ys.drop_left())))
      }
    }
    a => CalcitErr::err_str(format!("list:rest expected a list, got: {a}")),
  }
}

pub fn butlast(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes("butlast expected a list, got:", &xs);
  }
  match &xs[0] {
    Calcit::Nil => Ok(Calcit::Nil),
    Calcit::List(ys) => {
      if ys.is_empty() {
        Ok(Calcit::Nil)
      } else {
        Ok(Calcit::List(Arc::new(ys.butlast()?)))
      }
    }
    a => CalcitErr::err_str(format!("butlast expected a list, got: {a}")),
  }
}

pub fn concat(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  let mut ys = CalcitList::new_inner();
  for x in &xs {
    if let Calcit::List(zs) = x {
      for z in &zs.0 {
        ys = ys.push_right(z.to_owned());
      }
    } else {
      return CalcitErr::err_str(format!("concat expects list arguments, got: {x}"));
    }
  }
  Ok(Calcit::List(Arc::new(ys.into())))
}

pub fn range(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() || xs.len() > 3 {
    return CalcitErr::err_nodes("expected 1~3 arguments for range:", &xs);
  }
  let (base, bound) = match (&xs[0], xs.get(1)) {
    (Calcit::Number(bound), None) => (0.0, *bound),
    (Calcit::Number(base), Some(Calcit::Number(bound))) => (*base, *bound),
    (a, b) => return CalcitErr::err_str(format!("range expected base and bound, but got: {a} {b:?}")),
  };

  let step = match xs.get(2) {
    Some(Calcit::Number(n)) => *n,
    Some(a) => return CalcitErr::err_str(format!("range expected numbers, but got: {a}")),
    None => 1.0,
  };

  if (bound - base).abs() < f64::EPSILON {
    return Ok(Calcit::from(CalcitList::default()));
  }

  if step == 0.0 || (bound > base && step < 0.0) || (bound < base && step > 0.0) {
    return CalcitErr::err_str("range cannot construct list with step 0");
  }

  let mut ys = CalcitList::new_inner();
  let mut i = base;
  if step > 0.0 {
    while i < bound {
      ys = ys.push_right(Calcit::Number(i));
      i += step;
    }
  } else {
    while i > bound {
      ys = ys.push_right(Calcit::Number(i));
      i += step;
    }
  }
  Ok(Calcit::from(ys))
}

pub fn reverse(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes("butlast expected a list, got:", &xs);
  }
  match &xs[0] {
    Calcit::Nil => Ok(Calcit::Nil),
    Calcit::List(ys) => Ok(Calcit::from(ys.reverse())),
    a => CalcitErr::err_str(format!("butlast expected a list, got: {a}")),
  }
}

/// foldl using syntax for performance, it's supposed to be a function
pub fn foldl(xs: TernaryTreeList<Calcit>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() == 3 {
    let mut ret = xs[1].to_owned();

    match (&xs[0], &xs[2]) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn { info, .. }) => {
        for x in &xs.0 {
          let values = TernaryTreeList::from(&[ret, (*x).to_owned()]);
          ret = runner::run_fn(values, info, call_stack)?;
        }
        Ok(ret)
      }
      (Calcit::List(xs), Calcit::Proc(proc)) => {
        for x in &xs.0 {
          // println!("foldl args, {} {}", ret, x.to_owned());
          ret = builtins::handle_proc(*proc, TernaryTreeList::from(&[ret, (*x).to_owned()]), call_stack)?;
        }
        Ok(ret)
      }
      // also handles set
      (Calcit::Set(xs), Calcit::Fn { info, .. }) => {
        for x in xs {
          let values = TernaryTreeList::from(&[ret, x.to_owned()]);
          ret = runner::run_fn(values, info, call_stack)?;
        }
        Ok(ret)
      }
      (Calcit::Set(xs), Calcit::Proc(proc)) => {
        for x in xs {
          // println!("foldl args, {} {}", ret, x.to_owned());
          ret = builtins::handle_proc(*proc, TernaryTreeList::from(&[ret, x.to_owned()]), call_stack)?;
        }
        Ok(ret)
      }
      // also handles map
      (Calcit::Map(xs), Calcit::Fn { info, .. }) => {
        for (k, x) in xs {
          let values = TernaryTreeList::from(&[ret, Calcit::from(CalcitList::from(&[k.to_owned(), x.to_owned()]))]);
          ret = runner::run_fn(values, info, call_stack)?;
        }
        Ok(ret)
      }
      (Calcit::Map(xs), Calcit::Proc(proc)) => {
        for (k, x) in xs {
          // println!("foldl args, {} {}", ret, x.to_owned());
          ret = builtins::handle_proc(
            *proc,
            TernaryTreeList::from(&[ret, Calcit::from(CalcitList::from(&[k.to_owned(), x.to_owned()]))]),
            call_stack,
          )?;
        }
        Ok(ret)
      }

      (a, b) => Err(CalcitErr::use_msg_stack_location(
        format!("foldl expected list and function, got: {a} {b}"),
        call_stack,
        a.get_location().or_else(|| b.get_location()),
      )),
    }
  } else {
    Err(CalcitErr::use_msg_stack(
      format!("foldl expected 3 arguments, got: {}", Calcit::from(xs.to_owned())),
      call_stack,
    ))
  }
}

/// foldl-shortcut using syntax for performance, it's supposed to be a function
/// by returning `:: bool acc`, bool indicates where performace a shortcut return
pub fn foldl_shortcut(xs: TernaryTreeList<Calcit>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() == 4 {
    let acc = &xs[1];
    let default_value = &xs[2];
    match (&xs[0], &xs[3]) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn { info, .. }) => {
        let mut state = acc.to_owned();
        for x in &xs.0 {
          let values = TernaryTreeList::from(&[state, (*x).to_owned()]);
          let pair = runner::run_fn(values, info, call_stack)?;
          match pair {
            Calcit::Tuple(CalcitTuple { tag: x0, extra, .. }) => match &*x0 {
              Calcit::Bool(b) => {
                let x1 = extra.first().ok_or(CalcitErr::use_msg_stack_location(
                  "foldl-shortcut list expected value in tuple",
                  call_stack,
                  x0.get_location(),
                ))?;
                if *b {
                  return Ok((*x1).to_owned());
                } else {
                  state = (x1.to_owned()).to_owned()
                }
              }
              a => {
                return Err(CalcitErr::use_msg_stack_location(
                  format!("return value in foldl-shortcut should be a bool, got: {a}"),
                  call_stack,
                  a.get_location(),
                ))
              }
            },
            _ => {
              return Err(CalcitErr::use_msg_stack(
                format!("return value for foldl-shortcut should be `:: bool acc`, got: {pair}"),
                call_stack,
              ))
            }
          }
        }
        Ok(default_value.to_owned())
      }
      // almost identical body, except for the type
      (Calcit::Set(xs), Calcit::Fn { info, .. }) => {
        let mut state = acc.to_owned();
        for x in xs {
          let values = TernaryTreeList::from(&[state, x.to_owned()]);
          let pair = runner::run_fn(values, info, call_stack)?;
          match pair {
            Calcit::Tuple(CalcitTuple { tag: x0, extra, .. }) => match &*x0 {
              Calcit::Bool(b) => {
                let x1 = extra.first().ok_or(CalcitErr::use_msg_stack_location(
                  "foldl set expected value in tuple",
                  call_stack,
                  x0.get_location(),
                ))?;
                if *b {
                  return Ok((*x1).to_owned());
                } else {
                  state = (*x1).to_owned()
                }
              }
              a => {
                return Err(CalcitErr::use_msg_stack_location(
                  format!("return value in foldl-shortcut should be a bool, got: {a}"),
                  call_stack,
                  a.get_location(),
                ))
              }
            },
            _ => {
              return Err(CalcitErr::use_msg_stack(
                format!("return value for foldl-shortcut should be `:: bool acc`, got: {pair}"),
                call_stack,
              ))
            }
          }
        }
        Ok(default_value.to_owned())
      }
      // almost identical body, escept for the type
      (Calcit::Map(xs), Calcit::Fn { info, .. }) => {
        let mut state = acc.to_owned();
        for (k, x) in xs {
          let values = TernaryTreeList::from(&[state, Calcit::from(CalcitList::from(&[k.to_owned(), x.to_owned()]))]);
          let pair = runner::run_fn(values, info, call_stack)?;
          match pair {
            Calcit::Tuple(CalcitTuple { tag: x0, extra, .. }) => match &*x0 {
              Calcit::Bool(b) => {
                let x1 = extra.first().ok_or(CalcitErr::use_msg_stack_location(
                  "foldl map expected value in tuple",
                  call_stack,
                  x0.get_location(),
                ))?;
                if *b {
                  return Ok((*x1).to_owned());
                } else {
                  state = (*x1).to_owned()
                }
              }
              a => {
                return Err(CalcitErr::use_msg_stack_location(
                  format!("return value in foldl-shortcut should be a bool, got: {a}"),
                  call_stack,
                  a.get_location(),
                ))
              }
            },
            _ => {
              return Err(CalcitErr::use_msg_stack(
                format!("return value for foldl-shortcut should be `:: bool acc`, got: {pair}"),
                call_stack,
              ))
            }
          }
        }
        Ok(default_value.to_owned())
      }

      (a, b) => Err(CalcitErr::use_msg_stack_location(
        format!("foldl-shortcut expected list... and fn, got: {a} {b}"),
        call_stack,
        a.get_location().or_else(|| b.get_location()),
      )),
    }
  } else {
    Err(CalcitErr::use_msg_stack(
      format!(
        "foldl-shortcut expected 4 arguments list,state,default,fn, got: {}",
        Calcit::from(xs)
      ),
      call_stack,
    ))
  }
}

pub fn foldr_shortcut(xs: TernaryTreeList<Calcit>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() == 4 {
    // let xs = runner::evaluate_expr(&expr[0], scope, file_ns)?;
    let acc = &xs[1];
    let default_value = &xs[2];
    // let f = runner::evaluate_expr(&expr[3], scope, file_ns)?;
    match (&xs[0], &xs[3]) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn { info, .. }) => {
        let mut state = acc.to_owned();
        let size = xs.len();
        for i in 0..size {
          let x = xs.0[size - 1 - i].to_owned();
          let values = TernaryTreeList::from(&[state, x.to_owned()]);
          let pair = runner::run_fn(values, info, call_stack)?;
          match pair {
            Calcit::Tuple(CalcitTuple { tag: x0, extra, .. }) => match &*x0 {
              Calcit::Bool(b) => {
                let x1 = extra.first().ok_or(CalcitErr::use_msg_stack_location(
                  "foldl shortcut expected value in tuple",
                  call_stack,
                  x0.get_location(),
                ))?;
                if *b {
                  return Ok((*x1).to_owned());
                } else {
                  state = (*x1).to_owned()
                }
              }
              a => {
                return Err(CalcitErr::use_msg_stack_location(
                  format!("return value in foldr-shortcut should be a bool, got: {a}"),
                  call_stack,
                  a.get_location(),
                ))
              }
            },
            _ => {
              return Err(CalcitErr::use_msg_stack(
                format!("return value for foldr-shortcut should be `:: bool acc`, got: {pair}"),
                call_stack,
              ))
            }
          }
        }
        Ok(default_value.to_owned())
      }

      (a, b) => Err(CalcitErr::use_msg_stack_location(
        format!("foldr-shortcut expected list... and fn, got: {a} {b}"),
        call_stack,
        a.get_location().or_else(|| b.get_location()),
      )),
    }
  } else {
    Err(CalcitErr::use_msg_stack(
      format!(
        "foldr-shortcut expected 4 arguments list,state,default,fn, got: {}",
        CalcitList::from(xs)
      ),
      call_stack,
    ))
  }
}

pub fn sort(xs: TernaryTreeList<Calcit>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() == 2 {
    match (&xs[0], &xs[1]) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn { info, .. }) => {
        let mut xs2: Vec<Calcit> = vec![];
        for x in (**xs).into_iter() {
          xs2.push(x.to_owned())
        }
        xs2.sort_by(|a, b| -> Ordering {
          let values: TernaryTreeList<Calcit> = TernaryTreeList::from(&[(*a).to_owned(), (*b).to_owned()]);
          let v = runner::run_fn(values, info, call_stack);
          match v {
            Ok(Calcit::Number(x)) if x < 0.0 => Ordering::Less,
            Ok(Calcit::Number(x)) if x == 0.0 => Ordering::Equal,
            Ok(Calcit::Number(x)) if x > 0.0 => Ordering::Greater,
            Ok(a) => {
              eprintln!("expected number from sort comparator, got: {a}");
              panic!("failed to sort")
            }
            Err(e) => {
              eprintln!("sort failed, got: {e}");
              panic!("failed to sort")
            }
          }
        });
        let mut ys = CalcitList::new_inner();
        for x in xs2.iter() {
          // TODO ??
          ys = ys.push_right(x.to_owned())
        }
        Ok(Calcit::List(Arc::new(ys.into())))
      }
      (Calcit::List(xs), Calcit::Proc(proc)) => {
        let mut xs2: Vec<Calcit> = vec![];
        for x in (**xs).into_iter() {
          xs2.push(x.to_owned())
        }
        xs2.sort_by(|a, b| -> Ordering {
          let values = TernaryTreeList::from(&[(*a).to_owned(), (*b).to_owned()]);
          let v = builtins::handle_proc(*proc, values, call_stack);
          match v {
            Ok(Calcit::Number(x)) if x < 0.0 => Ordering::Less,
            Ok(Calcit::Number(x)) if x == 0.0 => Ordering::Equal,
            Ok(Calcit::Number(x)) if x > 0.0 => Ordering::Greater,
            Ok(a) => {
              eprintln!("expected number from sort comparator, got: {a}");
              panic!("failed to sort")
            }
            Err(e) => {
              eprintln!("sort failed, got: {e}");
              panic!("failed to sort")
            }
          }
        });
        let mut ys = CalcitList::new_inner();
        for x in xs2.iter() {
          // TODO ??
          ys = ys.push_right(x.to_owned())
        }
        Ok(Calcit::List(Arc::new(ys.into())))
      }
      (a, b) => Err(CalcitErr::use_msg_stack_location(
        format!("sort expected list and function, got: {a} {b}"),
        call_stack,
        a.get_location().or_else(|| b.get_location()),
      )),
    }
  } else {
    Err(CalcitErr::use_msg_stack(
      format!("sort expected 2 arguments, got: {}", Calcit::List(Arc::new(xs.into()))),
      call_stack,
    ))
  }
}

pub fn first(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_str("list:first expected 1 argument");
  }
  match &xs[0] {
    Calcit::List(ys) => {
      if ys.is_empty() {
        Ok(Calcit::Nil)
      } else {
        Ok((ys[0]).to_owned())
      }
    }
    a => CalcitErr::err_str(format!("list:first expected a list, got: {a}")),
  }
}

// real implementation relies of ternary-tree
pub fn assoc_before(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 3 {
    return CalcitErr::err_nodes("invalid arguments to assoc-before:", &xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::List(zs), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => {
        // let ys = insert(zs, idx, xs[2].to_owned());
        Ok(Calcit::List(Arc::new(zs.assoc_before(idx, xs[2].to_owned())?)))
      }
      Err(e) => CalcitErr::err_str(format!("assoc-before expect usize, {e}")),
    },
    (a, b) => CalcitErr::err_str(format!("assoc-before expected list and index, got: {a} {b}")),
  }
}

pub fn assoc_after(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 3 {
    return CalcitErr::err_nodes("invalid arguments to assoc-after:", &xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::List(zs), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => {
        // let ys = insert(zs, idx + 1, xs[2].to_owned());
        Ok(Calcit::from(zs.assoc_after(idx, xs[2].to_owned())?))
      }
      Err(e) => CalcitErr::err_str(format!("assoc-after expect usize, {e}")),
    },
    (a, b) => CalcitErr::err_str(format!("assoc-after expected list and index, got: {a} {b}")),
  }
}

pub fn empty_ques(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes("list empty? expected a list, got:", &xs);
  }
  match &xs[0] {
    Calcit::List(ys) => Ok(Calcit::Bool(ys.is_empty())),
    a => CalcitErr::err_str(format!("list empty? expected a list, got: {a}")),
  }
}

pub fn contains_ques(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes("list contains? expected list and a index, got:", &xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::List(xs), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => Ok(Calcit::Bool(idx < xs.len())),
      Err(_) => Ok(Calcit::Bool(false)),
    },
    (a, b) => CalcitErr::err_str(format!("list contains? expected list and iindex, got: {a} {b}")),
  }
}

pub fn includes_ques(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(xs)), Some(a)) => Ok(Calcit::Bool(xs.index_of(a).is_some())),
    (Some(a), ..) => CalcitErr::err_str(format!("list `includes?` expected list, list, got: {a}")),
    (None, ..) => CalcitErr::err_nodes("list `includes?` expected 2 arguments, got:", &xs),
  }
}

pub fn assoc(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 3 {
    return CalcitErr::err_nodes("list:assoc expected 3 arguments, got:", &xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::List(zs), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => {
        if idx < zs.len() {
          let mut ys: CalcitList = (**zs).to_owned();
          // ys[idx] = xs[2].to_owned();
          ys = ys.assoc(idx, xs[2].to_owned())?;
          Ok(Calcit::from(ys))
        } else {
          Ok(Calcit::List(Arc::new(xs.into())))
        }
      }
      Err(e) => CalcitErr::err_str(e),
    },
    (a, b) => CalcitErr::err_str(format!("list:assoc expected list and index, got: {a} {b}")),
  }
}

pub fn dissoc(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes("&list:dissoc expects 3 arguments, got:", &xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::List(xs), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(at) => Ok(Calcit::from(xs.dissoc(at)?)),
      Err(e) => CalcitErr::err_str(format!("dissoc expected number, {e}")),
    },
    (Calcit::List(_xs), a) => CalcitErr::err_str(format!("&list:dissoc expects a number in second argument, got {}", a)),
    (a, ..) => CalcitErr::err_str(format!("list dissoc expected a list, got: {a}")),
  }
}

pub fn list_to_set(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes("&list:to-set expected a single argument in list, got:", &xs);
  }
  match &xs[0] {
    Calcit::List(ys) => {
      let mut zs = rpds::HashTrieSet::new_sync();
      for y in &**ys {
        zs.insert_mut((*y).to_owned());
      }
      Ok(Calcit::Set(zs))
    }
    a => CalcitErr::err_str(format!("&list:to-set expected a list, got: {a}")),
  }
}

pub fn distinct(xs: TernaryTreeList<Calcit>) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes("&list:distinct expected a single argument in list, got:", &xs);
  }
  match &xs[0] {
    Calcit::List(ys) => {
      let mut zs = CalcitList::new_inner();
      for y in &**ys {
        if zs.index_of(y).is_none() {
          zs = zs.push_right((*y).to_owned());
        }
      }
      Ok(Calcit::from(CalcitList(zs)))
    }
    a => CalcitErr::err_str(format!("&list:distinct expected a list, got: {a}")),
  }
}
