use core::cmp::Ordering;
use std::sync::Arc;

use rpds::HashTrieSet;

use crate::calcit::{Calcit, CalcitErr, CalcitErrKind, CalcitList, CalcitTuple};
use crate::util::number::f64_to_usize;

use crate::builtins;
use crate::call_stack::CallStackList;
use crate::runner;

pub fn new_list(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  Ok(Calcit::List(Arc::new(xs.into())))
}

pub fn count(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&list:count expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::List(ys) => Ok(Calcit::Number(ys.len() as f64)),
    a => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:count expected a list, but received: {a}")),
  }
}

pub fn nth(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&list:nth expected 2 arguments, but received: {}", CalcitList::from(xs)),
    );
  }
  match (&xs[0], &xs[1]) {
    (Calcit::List(ys), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => match ys.get(idx) {
        Some(v) => Ok((*v).to_owned()),
        None => Ok(Calcit::Nil),
      },
      Err(e) => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:nth expected a valid index, {e}")),
    },
    (_, _) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&list:nth expected a list and an index, but received: {}", CalcitList::from(xs)),
    ),
  }
}

pub fn slice(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 && xs.len() != 3 {
    return CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&list:slice expected 2 or 3 arguments, but received: {}", CalcitList::from(xs)),
    );
  }
  match (&xs[0], &xs[1]) {
    (Calcit::List(ys), Calcit::Number(from)) => {
      let from_idx = f64_to_usize(*from)?;
      let to_idx = match xs.get(2) {
        Some(Calcit::Number(to)) => f64_to_usize(*to)?,
        Some(a) => {
          return CalcitErr::err_str(
            CalcitErrKind::Type,
            format!("&list:slice expected a number for index, but received: {a}"),
          );
        }
        None => ys.len(),
      };

      Ok(Calcit::List(Arc::new(ys.slice(from_idx, to_idx)?)))
    }
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&list:slice expected a list and numbers for indexes, but received: {a} {b}"),
    ),
  }
}

pub fn append(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&list:append expected 2 arguments, but received: {}", CalcitList::from(xs)),
    );
  }
  match &xs[0] {
    Calcit::List(ys) => Ok(Calcit::List(Arc::new(ys.push_right(xs[1].to_owned())))),
    a => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:append expected a list, but received: {a}")),
  }
}

pub fn prepend(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::List(ys)), Some(a)) => Ok(Calcit::List(Arc::new(ys.push_left(a.to_owned())))),
    (Some(a), _) => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:prepend expected a list, but received: {a}")),
    (None, _) => CalcitErr::err_str(CalcitErrKind::Arity, "&list:prepend expected 2 arguments, but received none"),
  }
}

pub fn rest(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&list:rest expected a list, but received:", xs);
  }
  match &xs[0] {
    Calcit::List(ys) => {
      if ys.is_empty() {
        Ok(Calcit::Nil)
      } else {
        Ok(Calcit::List(Arc::new(ys.drop_left())))
      }
    }
    a => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:rest expected a list, but received: {a}")),
  }
}

pub fn butlast(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&list:butlast expected a list, but received:", xs);
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
    a => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:butlast expected a list, but received: {a}")),
  }
}

pub fn concat(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let mut total_size = 0;
  for x in xs {
    if let Calcit::List(zs) = x {
      total_size += zs.len();
    } else {
      return CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&list:concat expected list arguments, but received: {x}"),
      );
    }
  }

  let mut ys = Vec::with_capacity(total_size);
  for x in xs {
    if let Calcit::List(zs) = x {
      // Use extend to efficiently append elements from the inner list
      ys.extend(zs.iter().map(|v| v.to_owned()));
    }
    // no need for else, already checked
  }
  Ok(Calcit::List(Arc::new(CalcitList::Vector(ys))))
}

pub fn range(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() || xs.len() > 3 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&list:range expected 1 to 3 arguments, but received:", xs);
  }
  let (base, bound) = match (&xs[0], xs.get(1)) {
    (Calcit::Number(bound), None) => (0.0, *bound),
    (Calcit::Number(base), Some(Calcit::Number(bound))) => (*base, *bound),
    (a, b) => {
      return CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&list:range expected numbers for base and bound, but received: {a} {b:?}"),
      );
    }
  };

  let step = match xs.get(2) {
    Some(Calcit::Number(n)) => *n,
    Some(a) => {
      return CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&list:range expected a number for step, but received: {a}"),
      );
    }
    None => 1.0,
  };

  if (bound - base).abs() < f64::EPSILON {
    return Ok(Calcit::from(CalcitList::default()));
  }

  if step == 0.0 || (bound > base && step < 0.0) || (bound < base && step > 0.0) {
    return CalcitErr::err_str(
      CalcitErrKind::Unexpected,
      "&list:range cannot construct list with a step of 0 or invalid step direction",
    );
  }

  let mut ys = vec![];
  let mut i = base;
  if step > 0.0 {
    while i < bound {
      ys.push(Calcit::Number(i));
      i += step;
    }
  } else {
    while i > bound {
      ys.push(Calcit::Number(i));
      i += step;
    }
  }
  Ok(Calcit::from(ys))
}

pub fn reverse(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&list:reverse expected a list, but received:", xs);
  }
  match &xs[0] {
    Calcit::Nil => Ok(Calcit::Nil),
    Calcit::List(ys) => Ok(Calcit::from(ys.reverse())),
    a => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:reverse expected a list, but received: {a}")),
  }
}

/// foldl using syntax for performance, it's supposed to be a function
pub fn foldl(xs: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() == 3 {
    let mut ret = xs[1].to_owned();

    match (&xs[0], &xs[2]) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn { info, .. }) => {
        for x in xs.iter() {
          ret = runner::run_fn(&[ret, (*x).to_owned()], info, call_stack)?;
        }
        Ok(ret)
      }
      (Calcit::List(xs), Calcit::Proc(proc)) => {
        for x in xs.iter() {
          // println!("foldl args, {} {}", ret, x.to_owned());
          ret = builtins::handle_proc(*proc, &[ret, (*x).to_owned()], call_stack)?;
        }
        Ok(ret)
      }
      // also handles set
      (Calcit::Set(xs), Calcit::Fn { info, .. }) => {
        for x in xs {
          ret = runner::run_fn(&[ret, x.to_owned()], info, call_stack)?;
        }
        Ok(ret)
      }
      (Calcit::Set(xs), Calcit::Proc(proc)) => {
        for x in xs {
          // println!("foldl args, {} {}", ret, x.to_owned());
          ret = builtins::handle_proc(*proc, &[ret, x.to_owned()], call_stack)?;
        }
        Ok(ret)
      }
      // also handles map
      (Calcit::Map(xs), Calcit::Fn { info, .. }) => {
        for (k, x) in xs {
          ret = runner::run_fn(
            &[ret, Calcit::from(CalcitList::from(&[k.to_owned(), x.to_owned()]))],
            info,
            call_stack,
          )?;
        }
        Ok(ret)
      }
      (Calcit::Map(xs), Calcit::Proc(proc)) => {
        for (k, x) in xs {
          // println!("foldl args, {} {}", ret, x.to_owned());
          ret = builtins::handle_proc(
            *proc,
            &[ret, Calcit::from(CalcitList::from(&[k.to_owned(), x.to_owned()]))],
            call_stack,
          )?;
        }
        Ok(ret)
      }

      (a, b) => Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Type,
        format!("&list:foldl expected a list and a function, but received: {a} {b}"),
        call_stack,
        a.get_location().or_else(|| b.get_location()),
      )),
    }
  } else {
    Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Arity,
      format!("&list:foldl expected 3 arguments, but received: {}", CalcitList::from(xs)),
      call_stack,
    ))
  }
}

/// foldl-shortcut using syntax for performance, it's supposed to be a function
/// by returning `:: bool acc`, bool indicates where performace a shortcut return
pub fn foldl_shortcut(xs: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() == 4 {
    let acc = &xs[1];
    let default_value = &xs[2];
    match (&xs[0], &xs[3]) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn { info, .. }) => {
        let mut state = acc.to_owned();
        for x in xs.iter() {
          let pair = runner::run_fn(&[state.to_owned(), (*x).to_owned()], info, call_stack)?;
          match pair {
            Calcit::Tuple(CalcitTuple { tag: x0, extra, .. }) => match &*x0 {
              Calcit::Bool(b) => {
                let x1 = extra.first().ok_or(CalcitErr::use_msg_stack_location(
                  CalcitErrKind::Arity,
                  "&list:foldl-shortcut expected a value in the tuple",
                  call_stack,
                  x0.get_location(),
                ))?;
                if *b {
                  return Ok((*x1).to_owned());
                } else {
                  x1.clone_into(&mut state)
                }
              }
              a => {
                return Err(CalcitErr::use_msg_stack_location(
                  CalcitErrKind::Type,
                  format!("&list:foldl-shortcut return value must be a boolean, but received: {a}"),
                  call_stack,
                  a.get_location(),
                ));
              }
            },
            _ => {
              return Err(CalcitErr::use_msg_stack(
                CalcitErrKind::Type,
                format!("&list:foldl-shortcut return value must be `:: boolean accumulator`, but received: {pair}"),
                call_stack,
              ));
            }
          }
        }
        Ok(default_value.to_owned())
      }
      // almost identical body, except for the type
      (Calcit::Set(xs), Calcit::Fn { info, .. }) => {
        let mut state = acc.to_owned();
        for x in xs {
          let pair = runner::run_fn(&[state.to_owned(), x.to_owned()], info, call_stack)?;
          match pair {
            Calcit::Tuple(CalcitTuple { tag: x0, extra, .. }) => match &*x0 {
              Calcit::Bool(b) => {
                let x1 = extra.first().ok_or(CalcitErr::use_msg_stack_location(
                  CalcitErrKind::Arity,
                  "&list:foldl-shortcut expected a value in the tuple",
                  call_stack,
                  x0.get_location(),
                ))?;
                if *b {
                  return Ok((*x1).to_owned());
                } else {
                  x1.clone_into(&mut state)
                }
              }
              a => {
                return Err(CalcitErr::use_msg_stack_location(
                  CalcitErrKind::Type,
                  format!("&list:foldl-shortcut return value must be a boolean, but received: {a}"),
                  call_stack,
                  a.get_location(),
                ));
              }
            },
            _ => {
              return Err(CalcitErr::use_msg_stack(
                CalcitErrKind::Type,
                format!("&list:foldl-shortcut return value must be `:: boolean accumulator`, but received: {pair}"),
                call_stack,
              ));
            }
          }
        }
        Ok(default_value.to_owned())
      }
      // almost identical body, escept for the type
      (Calcit::Map(xs), Calcit::Fn { info, .. }) => {
        let mut state = acc.to_owned();
        for (k, x) in xs {
          let pair = runner::run_fn(
            &[state.to_owned(), Calcit::from(CalcitList::from(&[k.to_owned(), x.to_owned()]))],
            info,
            call_stack,
          )?;
          match pair {
            Calcit::Tuple(CalcitTuple { tag: x0, extra, .. }) => match &*x0 {
              Calcit::Bool(b) => {
                let x1 = extra.first().ok_or(CalcitErr::use_msg_stack_location(
                  CalcitErrKind::Arity,
                  "&list:foldl-shortcut expected a value in the tuple",
                  call_stack,
                  x0.get_location(),
                ))?;
                if *b {
                  return Ok((*x1).to_owned());
                } else {
                  (*x1).clone_into(&mut state)
                }
              }
              a => {
                return Err(CalcitErr::use_msg_stack_location(
                  CalcitErrKind::Type,
                  format!("&list:foldl-shortcut return value must be a boolean, but received: {a}"),
                  call_stack,
                  a.get_location(),
                ));
              }
            },
            _ => {
              return Err(CalcitErr::use_msg_stack(
                CalcitErrKind::Type,
                format!("&list:foldl-shortcut return value must be `:: boolean accumulator`, but received: {pair}"),
                call_stack,
              ));
            }
          }
        }
        Ok(default_value.to_owned())
      }

      (a, b) => Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Type,
        format!("&list:foldl-shortcut expected a list and a function, but received: {a} {b}"),
        call_stack,
        a.get_location().or_else(|| b.get_location()),
      )),
    }
  } else {
    Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Arity,
      format!(
        "&list:foldl-shortcut expected 4 arguments (list, state, default, fn), but received: {}",
        CalcitList::from(xs)
      ),
      call_stack,
    ))
  }
}

pub fn foldr_shortcut(xs: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
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
          let x = xs[size - 1 - i].to_owned();
          let pair = runner::run_fn(&[state.to_owned(), x.to_owned()], info, call_stack)?;
          match pair {
            Calcit::Tuple(CalcitTuple { tag: x0, extra, .. }) => match &*x0 {
              Calcit::Bool(b) => {
                let x1 = extra.first().ok_or(CalcitErr::use_msg_stack_location(
                  CalcitErrKind::Arity,
                  "&list:foldr-shortcut expected a value in the tuple",
                  call_stack,
                  x0.get_location(),
                ))?;
                if *b {
                  return Ok((*x1).to_owned());
                } else {
                  (*x1).clone_into(&mut state)
                }
              }
              a => {
                return Err(CalcitErr::use_msg_stack_location(
                  CalcitErrKind::Type,
                  format!("&list:foldr-shortcut return value must be a boolean, but received: {a}"),
                  call_stack,
                  a.get_location(),
                ));
              }
            },
            _ => {
              return Err(CalcitErr::use_msg_stack(
                CalcitErrKind::Type,
                format!("&list:foldr-shortcut return value must be `:: boolean accumulator`, but received: {pair}"),
                call_stack,
              ));
            }
          }
        }
        Ok(default_value.to_owned())
      }

      (a, b) => Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Type,
        format!("&list:foldr-shortcut expected a list and a function, but received: {a} {b}"),
        call_stack,
        a.get_location().or_else(|| b.get_location()),
      )),
    }
  } else {
    Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Arity,
      format!(
        "&list:foldr-shortcut expected 4 arguments (list, state, default, fn), but received: {}",
        CalcitList::from(xs)
      ),
      call_stack,
    ))
  }
}

pub fn sort(xs: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() == 2 {
    match (&xs[0], &xs[1]) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn { info, .. }) => {
        let mut xs2: Vec<Calcit> = xs.to_vec(); // Use existing to_vec()
        xs2.sort_by(|a, b| -> Ordering {
          let v = runner::run_fn(&[(*a).to_owned(), (*b).to_owned()], info, call_stack);
          match v {
            Ok(Calcit::Number(x)) if x < 0.0 => Ordering::Less,
            Ok(Calcit::Number(x)) if x > 0.0 => Ordering::Greater,
            Ok(Calcit::Number(_)) => Ordering::Equal,
            Ok(a) => {
              eprintln!("&list:sort comparator must return a number, but received: {a}");
              panic!("failed to sort")
            }
            Err(e) => {
              eprintln!("&list:sort failed: {e}");
              panic!("failed to sort")
            }
          }
        });
        Ok(Calcit::List(Arc::new(CalcitList::Vector(xs2))))
      }
      (Calcit::List(xs), Calcit::Proc(proc)) => {
        let mut xs2: Vec<Calcit> = xs.to_vec(); // Use existing to_vec()
        xs2.sort_by(|a, b| -> Ordering {
          let v = builtins::handle_proc(*proc, &[(*a).to_owned(), (*b).to_owned()], call_stack);
          match v {
            Ok(Calcit::Number(x)) if x < 0.0 => Ordering::Less,
            Ok(Calcit::Number(x)) if x > 0.0 => Ordering::Greater,
            Ok(Calcit::Number(_)) => Ordering::Equal,
            Ok(a) => {
              eprintln!("&list:sort comparator must return a number, but received: {a}");
              panic!("failed to sort")
            }
            Err(e) => {
              eprintln!("&list:sort failed: {e}");
              panic!("failed to sort")
            }
          }
        });
        Ok(Calcit::List(Arc::new(CalcitList::Vector(xs2)))) // Directly create from Vec
      }
      (a, b) => Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Type,
        format!("&list:sort expected a list and a function, but received: {a} {b}"),
        call_stack,
        a.get_location().or_else(|| b.get_location()),
      )),
    }
  } else {
    Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Arity,
      format!(
        "&list:sort expected 2 arguments, but received: {}",
        Calcit::List(Arc::new(xs.into()))
      ),
      call_stack,
    ))
  }
}

pub fn first(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_str(CalcitErrKind::Arity, "&list:first expected 1 argument, but received none");
  }
  match &xs[0] {
    Calcit::List(ys) => {
      if ys.is_empty() {
        Ok(Calcit::Nil)
      } else {
        Ok((ys[0]).to_owned())
      }
    }
    a => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:first expected a list, but received: {a}")),
  }
}

// real implementation relies of ternary-tree
pub fn assoc_before(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 3 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&list:assoc-before expected 3 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::List(zs), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => {
        // let ys = insert(zs, idx, xs[2].to_owned());
        Ok(Calcit::List(Arc::new(zs.assoc_before(idx, xs[2].to_owned())?)))
      }
      Err(e) => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:assoc-before expected a valid index, {e}")),
    },
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&list:assoc-before expected a list and an index, but received: {a} {b}"),
    ),
  }
}

pub fn assoc_after(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 3 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&list:assoc-after expected 3 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::List(zs), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => {
        // let ys = insert(zs, idx + 1, xs[2].to_owned());
        Ok(Calcit::from(zs.assoc_after(idx, xs[2].to_owned())?))
      }
      Err(e) => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:assoc-after expected a valid index, {e}")),
    },
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&list:assoc-after expected a list and an index, but received: {a} {b}"),
    ),
  }
}

pub fn empty_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&list:empty? expected a list, but received:", xs);
  }
  match &xs[0] {
    Calcit::List(ys) => Ok(Calcit::Bool(ys.is_empty())),
    a => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:empty? expected a list, but received: {a}")),
  }
}

pub fn contains_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&list:contains? expected a list and an index, but received:",
      xs,
    );
  }
  match (&xs[0], &xs[1]) {
    (Calcit::List(xs), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => Ok(Calcit::Bool(idx < xs.len())),
      Err(_) => Ok(Calcit::Bool(false)),
    },
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&list:contains? expected a list and an index, but received: {a} {b}"),
    ),
  }
}

pub fn includes_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::List(xs)), Some(a)) => Ok(Calcit::Bool(xs.index_of(a).is_some())),
    (Some(a), ..) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&list:includes? expected a list and a value, but received: {a}"),
    ),
    (None, ..) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&list:includes? expected 2 arguments, but received:", xs),
  }
}

pub fn assoc(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 3 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&list:assoc expected 3 arguments, but received:", xs);
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
      Err(e) => CalcitErr::err_str(CalcitErrKind::Type, e),
    },
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&list:assoc expected a list and an index, but received: {a} {b}"),
    ),
  }
}

pub fn dissoc(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&list:dissoc expected 2 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::List(xs), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(at) => Ok(Calcit::from(xs.dissoc(at)?)),
      Err(e) => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:dissoc expected a valid index, {e}")),
    },
    (Calcit::List(_xs), a) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&list:dissoc expected a number for index, but received: {a}"),
    ),
    (a, ..) => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:dissoc expected a list, but received: {a}")),
  }
}

pub fn list_to_set(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&list:to-set expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::List(ys) => {
      let mut zs = rpds::HashTrieSet::new_sync();
      ys.traverse(&mut |y| {
        zs.insert_mut(y.to_owned());
      });
      Ok(Calcit::Set(zs))
    }
    a => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:to-set expected a list, but received: {a}")),
  }
}

pub fn distinct(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&list:distinct expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::List(ys) => {
      let mut seen = HashTrieSet::new_sync();
      let mut zs = CalcitList::new_inner();
      ys.traverse(&mut |y| {
        if !seen.contains(y) {
          seen.insert_mut(y.to_owned());
          zs = zs.push_right(y.to_owned());
        }
      });
      Ok(Calcit::from(CalcitList::List(zs)))
    }
    a => CalcitErr::err_str(CalcitErrKind::Type, format!("&list:distinct expected a list, but received: {a}")),
  }
}
