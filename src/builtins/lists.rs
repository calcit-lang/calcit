use core::cmp::Ordering;

use crate::primes::{Calcit, CalcitErr, CalcitItems, CrListWrap};
use crate::util::number::f64_to_usize;

use crate::builtins;
use crate::call_stack::CallStackVec;
use crate::runner;

pub fn new_list(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  Ok(Calcit::List(xs.to_owned()))
}

pub fn count(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::List(ys)) => Ok(Calcit::Number(ys.len() as f64)),
    Some(a) => Err(CalcitErr::use_string(format!("list count expected a list, got: {}", a))),
    None => Err(CalcitErr::use_str("list count expected 1 argument")),
  }
}

pub fn nth(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(ys)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => match ys.get(idx) {
        Some(v) => Ok(v.to_owned()),
        None => Ok(Calcit::Nil),
      },
      Err(e) => Err(CalcitErr::use_string(format!("nth expect usize, {}", e))),
    },
    (Some(_), None) => Err(CalcitErr::use_string(format!(
      "string nth expected a list and index, got: {:?}",
      xs
    ))),
    (None, Some(_)) => Err(CalcitErr::use_string(format!(
      "string nth expected a list and index, got: {:?}",
      xs
    ))),
    (_, _) => Err(CalcitErr::use_string(format!(
      "nth expected 2 argument, got: {}",
      CrListWrap(xs.to_owned())
    ))),
  }
}

pub fn slice(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(ys)), Some(Calcit::Number(from))) => {
      let to_idx = match xs.get(2) {
        Some(Calcit::Number(to)) => {
          let idx: usize = unsafe { to.to_int_unchecked() };
          idx
        }
        Some(a) => return Err(CalcitErr::use_string(format!("slice expected number index, got: {}", a))),
        None => ys.len(),
      };
      let from_idx: usize = unsafe { from.to_int_unchecked() };
      Ok(Calcit::List(ys.to_owned().slice(from_idx..to_idx)))
    }
    (Some(Calcit::List(_)), Some(a)) => Err(CalcitErr::use_string(format!("slice expected index number, got: {}", a))),
    (Some(Calcit::List(_)), None) => Err(CalcitErr::use_str("slice expected index numbers")),
    (_, _) => Err(CalcitErr::use_str("slice expected 2~3 arguments")),
  }
}

pub fn append(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(ys)), Some(a)) => {
      let mut zs = ys.to_owned();
      zs.push_back(a.to_owned());
      Ok(Calcit::List(zs))
    }
    (Some(a), _) => Err(CalcitErr::use_string(format!("append expected list, got: {}", a))),
    (None, _) => Err(CalcitErr::use_str("append expected 2 arguments, got nothing")),
  }
}

pub fn prepend(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(ys)), Some(a)) => {
      let mut zs = ys.to_owned();
      zs.push_front(a.to_owned());
      Ok(Calcit::List(zs))
    }
    (Some(a), _) => Err(CalcitErr::use_string(format!("prepend expected list, got: {}", a))),
    (None, _) => Err(CalcitErr::use_str("prepend expected 2 arguments, got nothing")),
  }
}

pub fn rest(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::List(ys)) => {
      if ys.is_empty() {
        Ok(Calcit::Nil)
      } else {
        let mut zs = ys.to_owned();
        zs.pop_front();
        Ok(Calcit::List(zs))
      }
    }
    Some(a) => Err(CalcitErr::use_string(format!("list:rest expected a list, got: {}", a))),
    None => Err(CalcitErr::use_str("list:rest expected 1 argument")),
  }
}

pub fn butlast(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Nil),
    Some(Calcit::List(ys)) => {
      if ys.is_empty() {
        Ok(Calcit::Nil)
      } else {
        let mut zs = ys.to_owned();
        zs.pop_back();
        Ok(Calcit::List(zs))
      }
    }
    Some(a) => Err(CalcitErr::use_string(format!("butlast expected a list, got: {}", a))),
    None => Err(CalcitErr::use_str("butlast expected 1 argument")),
  }
}

pub fn concat(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  let mut ys: CalcitItems = im::vector![];
  for x in xs {
    if let Calcit::List(zs) = x {
      for z in zs {
        ys.push_back(z.to_owned());
      }
    } else {
      return Err(CalcitErr::use_string(format!("concat expects list arguments, got: {}", x)));
    }
  }
  Ok(Calcit::List(ys))
}

pub fn range(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  let (base, bound) = match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(bound)), None) => (0.0, *bound),
    (Some(Calcit::Number(base)), Some(Calcit::Number(bound))) => (*base, *bound),
    (Some(a), Some(b)) => return Err(CalcitErr::use_string(format!("range expected 2 numbers, but got: {} {}", a, b))),
    (_, _) => return Err(CalcitErr::use_string(format!("invalid arguments for range: {:?}", xs))),
  };

  let step = match xs.get(2) {
    Some(Calcit::Number(n)) => *n,
    Some(a) => return Err(CalcitErr::use_string(format!("range expected numbers, but got: {}", a))),
    None => 1.0,
  };

  if (bound - base).abs() < f64::EPSILON {
    return Ok(Calcit::List(im::vector![]));
  }

  if step == 0.0 || (bound > base && step < 0.0) || (bound < base && step > 0.0) {
    return Err(CalcitErr::use_str("range cannot construct list with step 0"));
  }

  let mut ys: CalcitItems = im::vector![];
  let mut i = base;
  if step > 0.0 {
    while i < bound {
      ys.push_back(Calcit::Number(i));
      i += step;
    }
  } else {
    while i > bound {
      ys.push_back(Calcit::Number(i));
      i += step;
    }
  }
  Ok(Calcit::List(ys))
}

pub fn reverse(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Nil),
    Some(Calcit::List(ys)) => {
      let mut zs: CalcitItems = im::vector![];
      for y in ys {
        zs.push_front(y.to_owned());
      }
      Ok(Calcit::List(zs))
    }
    Some(a) => Err(CalcitErr::use_string(format!("butlast expected a list, got: {}", a))),
    None => Err(CalcitErr::use_str("butlast expected 1 argument")),
  }
}

/// foldl using syntax for performance, it's supposed to be a function
pub fn foldl(xs: &CalcitItems, call_stack: &CallStackVec) -> Result<Calcit, CalcitErr> {
  if xs.len() == 3 {
    let mut ret = xs[1].to_owned();

    match (&xs[0], &xs[2]) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        for x in xs {
          let values = im::vector![ret, x.to_owned()];
          ret = runner::run_fn(&values, def_scope, args, body, def_ns, call_stack)?;
        }
        Ok(ret)
      }
      (Calcit::List(xs), Calcit::Proc(proc)) => {
        for x in xs {
          // println!("foldl args, {} {}", ret, x.to_owned());
          ret = builtins::handle_proc(proc, &im::vector![ret, x.to_owned()], call_stack)?;
        }
        Ok(ret)
      }
      // also handles set
      (Calcit::Set(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        for x in xs {
          let values = im::vector![ret, x.to_owned()];
          ret = runner::run_fn(&values, def_scope, args, body, def_ns, call_stack)?;
        }
        Ok(ret)
      }
      (Calcit::Set(xs), Calcit::Proc(proc)) => {
        for x in xs {
          // println!("foldl args, {} {}", ret, x.to_owned());
          ret = builtins::handle_proc(proc, &im::vector![ret, x.to_owned()], call_stack)?;
        }
        Ok(ret)
      }
      // also handles map
      (Calcit::Map(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        for (k, x) in xs {
          let values = im::vector![ret, Calcit::List(im::vector![k.to_owned(), x.to_owned()])];
          ret = runner::run_fn(&values, def_scope, args, body, def_ns, call_stack)?;
        }
        Ok(ret)
      }
      (Calcit::Map(xs), Calcit::Proc(proc)) => {
        for (k, x) in xs {
          // println!("foldl args, {} {}", ret, x.to_owned());
          ret = builtins::handle_proc(
            proc,
            &im::vector![ret, Calcit::List(im::vector![k.to_owned(), x.to_owned()])],
            call_stack,
          )?;
        }
        Ok(ret)
      }

      (a, b) => Err(CalcitErr::use_msg_stack(
        format!("foldl expected list and function, got: {} {}", a, b),
        call_stack,
      )),
    }
  } else {
    Err(CalcitErr::use_msg_stack(
      format!("foldl expected 3 arguments, got: {:?}", xs),
      call_stack,
    ))
  }
}

/// foldl-shortcut using syntax for performance, it's supposed to be a function
/// by returning `:: bool acc`, bool indicates where performace a shortcut return
pub fn foldl_shortcut(xs: &CalcitItems, call_stack: &CallStackVec) -> Result<Calcit, CalcitErr> {
  if xs.len() == 4 {
    let acc = &xs[1];
    let default_value = &xs[2];
    match (&xs[0], &xs[3]) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        let mut state = acc.to_owned();
        for x in xs {
          let values = im::vector![state, x.to_owned()];
          let pair = runner::run_fn(&values, def_scope, args, body, def_ns, call_stack)?;
          match pair {
            Calcit::Tuple(x0, x1) => match *x0 {
              Calcit::Bool(b) => {
                if b {
                  return Ok(*x1);
                } else {
                  state = *x1.to_owned()
                }
              }
              a => {
                return Err(CalcitErr::use_msg_stack(
                  format!("return value in foldl-shortcut should be a bool, got: {}", a),
                  call_stack,
                ))
              }
            },
            _ => {
              return Err(CalcitErr::use_msg_stack(
                format!("return value for foldl-shortcut should be `:: bool acc`, got: {}", pair),
                call_stack,
              ))
            }
          }
        }
        Ok(default_value.to_owned())
      }
      // almost identical body, escept for the type
      (Calcit::Set(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        let mut state = acc.to_owned();
        for x in xs {
          let values = im::vector![state, x.to_owned()];
          let pair = runner::run_fn(&values, def_scope, args, body, def_ns, call_stack)?;
          match pair {
            Calcit::Tuple(x0, x1) => match *x0 {
              Calcit::Bool(b) => {
                if b {
                  return Ok(*x1);
                } else {
                  state = *x1.to_owned()
                }
              }
              a => {
                return Err(CalcitErr::use_msg_stack(
                  format!("return value in foldl-shortcut should be a bool, got: {}", a),
                  call_stack,
                ))
              }
            },
            _ => {
              return Err(CalcitErr::use_msg_stack(
                format!("return value for foldl-shortcut should be `:: bool acc`, got: {}", pair),
                call_stack,
              ))
            }
          }
        }
        Ok(default_value.to_owned())
      }
      // almost identical body, escept for the type
      (Calcit::Map(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        let mut state = acc.to_owned();
        for (k, x) in xs {
          let values = im::vector![state, Calcit::List(im::vector![k.to_owned(), x.to_owned()])];
          let pair = runner::run_fn(&values, def_scope, args, body, def_ns, call_stack)?;
          match pair {
            Calcit::Tuple(x0, x1) => match *x0 {
              Calcit::Bool(b) => {
                if b {
                  return Ok(*x1);
                } else {
                  state = *x1.to_owned()
                }
              }
              a => {
                return Err(CalcitErr::use_msg_stack(
                  format!("return value in foldl-shortcut should be a bool, got: {}", a),
                  call_stack,
                ))
              }
            },
            _ => {
              return Err(CalcitErr::use_msg_stack(
                format!("return value for foldl-shortcut should be `:: bool acc`, got: {}", pair),
                call_stack,
              ))
            }
          }
        }
        Ok(default_value.to_owned())
      }

      (a, b) => Err(CalcitErr::use_msg_stack(
        format!("foldl-shortcut expected list... and fn, got: {} {}", a, b),
        call_stack,
      )),
    }
  } else {
    Err(CalcitErr::use_msg_stack(
      format!("foldl-shortcut expected 4 arguments list,state,default,fn, got: {:?}", xs),
      call_stack,
    ))
  }
}

pub fn foldr_shortcut(xs: &CalcitItems, call_stack: &CallStackVec) -> Result<Calcit, CalcitErr> {
  if xs.len() == 4 {
    // let xs = runner::evaluate_expr(&expr[0], scope, file_ns)?;
    let acc = &xs[1];
    let default_value = &xs[2];
    // let f = runner::evaluate_expr(&expr[3], scope, file_ns)?;
    match (&xs[0], &xs[3]) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        let mut state = acc.to_owned();
        let size = xs.len();
        for i in 0..size {
          let x = xs[size - 1 - i].to_owned();
          let values = im::vector![state, x];
          let pair = runner::run_fn(&values, def_scope, args, body, def_ns, call_stack)?;
          match pair {
            Calcit::Tuple(x0, x1) => match *x0 {
              Calcit::Bool(b) => {
                if b {
                  return Ok(*x1);
                } else {
                  state = *x1.to_owned()
                }
              }
              a => {
                return Err(CalcitErr::use_msg_stack(
                  format!("return value in foldr-shortcut should be a bool, got: {}", a),
                  call_stack,
                ))
              }
            },
            _ => {
              return Err(CalcitErr::use_msg_stack(
                format!("return value for foldr-shortcut should be `:: bool acc`, got: {}", pair),
                call_stack,
              ))
            }
          }
        }
        Ok(default_value.to_owned())
      }

      (a, b) => Err(CalcitErr::use_msg_stack(
        format!("foldr-shortcut expected list... and fn, got: {} {}", a, b),
        call_stack,
      )),
    }
  } else {
    Err(CalcitErr::use_msg_stack(
      format!("foldr-shortcut expected 4 arguments list,state,default,fn, got: {:?}", xs),
      call_stack,
    ))
  }
}

pub fn sort(xs: &CalcitItems, call_stack: &CallStackVec) -> Result<Calcit, CalcitErr> {
  if xs.len() == 2 {
    match (&xs[0], &xs[1]) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        let mut ret = xs.to_owned();
        ret.sort_by(|a, b| {
          let values = im::vector![a.to_owned(), b.to_owned()];
          let v = runner::run_fn(&values, def_scope, args, body, def_ns, call_stack);
          match v {
            Ok(Calcit::Number(x)) if x < 0.0 => Ordering::Less,
            Ok(Calcit::Number(x)) if x == 0.0 => Ordering::Equal,
            Ok(Calcit::Number(x)) if x > 0.0 => Ordering::Greater,
            Ok(a) => {
              println!("expected number from sort comparator, got: {}", a);
              panic!("failed to sort")
            }
            Err(e) => {
              println!("sort failed, got: {}", e);
              panic!("failed to sort")
            }
          }
        });
        Ok(Calcit::List(ret))
      }
      (Calcit::List(xs), Calcit::Proc(proc)) => {
        let mut ret = xs.to_owned();
        ret.sort_by(|a, b| {
          let values = im::vector![a.to_owned(), b.to_owned()];
          let v = builtins::handle_proc(proc, &values, call_stack);
          match v {
            Ok(Calcit::Number(x)) if x < 0.0 => Ordering::Less,
            Ok(Calcit::Number(x)) if x == 0.0 => Ordering::Equal,
            Ok(Calcit::Number(x)) if x > 0.0 => Ordering::Greater,
            Ok(a) => {
              println!("expected number from sort comparator, got: {}", a);
              panic!("failed to sort")
            }
            Err(e) => {
              println!("sort failed, got: {}", e);
              panic!("failed to sort")
            }
          }
        });
        Ok(Calcit::List(ret))
      }

      (a, b) => Err(CalcitErr::use_msg_stack(
        format!("sort expected list and function, got: {} {}", a, b),
        call_stack,
      )),
    }
  } else {
    Err(CalcitErr::use_msg_stack(
      format!("sort expected 2 arguments, got: {:?}", xs),
      call_stack,
    ))
  }
}

pub fn first(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::List(ys)) => {
      if ys.is_empty() {
        Ok(Calcit::Nil)
      } else {
        Ok(ys[0].to_owned())
      }
    }
    Some(a) => Err(CalcitErr::use_string(format!("list:first expected a list, got: {}", a))),
    None => Err(CalcitErr::use_str("list:first expected 1 argument")),
  }
}

// real implementation relies of ternary-tree
pub fn assoc_before(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n)), Some(a)) => match f64_to_usize(*n) {
      Ok(idx) => {
        let mut ys = xs.to_owned();
        ys.insert(idx, a.to_owned());
        Ok(Calcit::List(ys))
      }
      Err(e) => Err(CalcitErr::use_string(format!("assoc-before expect usize, {}", e))),
    },
    (Some(a), Some(b), Some(c)) => Err(CalcitErr::use_string(format!(
      "assoc-before expected list and index, got: {} {} {}",
      a, b, c
    ))),
    (a, b, c) => Err(CalcitErr::use_string(format!(
      "invalid arguments to assoc-before: {:?} {:?} {:?}",
      a, b, c
    ))),
  }
}

pub fn assoc_after(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n)), Some(a)) => match f64_to_usize(*n) {
      Ok(idx) => {
        let mut ys = xs.to_owned();
        ys.insert(idx + 1, a.to_owned());
        Ok(Calcit::List(ys))
      }
      Err(e) => Err(CalcitErr::use_string(format!("assoc-after expect usize, {}", e))),
    },
    (Some(a), Some(b), Some(c)) => Err(CalcitErr::use_string(format!(
      "assoc-after expected list and index, got: {} {} {}",
      a, b, c
    ))),
    (a, b, c) => Err(CalcitErr::use_string(format!(
      "invalid arguments to assoc-after: {:?} {:?} {:?}",
      a, b, c
    ))),
  }
}

pub fn empty_ques(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::List(ys)) => Ok(Calcit::Bool(ys.is_empty())),
    Some(a) => Err(CalcitErr::use_string(format!("list empty? expected a list, got: {}", a))),
    None => Err(CalcitErr::use_str("list empty? expected 1 argument")),
  }
}

pub fn contains_ques(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => Ok(Calcit::Bool(idx < xs.len())),
      Err(_) => Ok(Calcit::Bool(false)),
    },
    (Some(a), ..) => Err(CalcitErr::use_string(format!("list contains? expected list, got: {}", a))),
    (None, ..) => Err(CalcitErr::use_string(format!("list contains? expected 2 arguments, got: {:?}", xs))),
  }
}

pub fn includes_ques(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains(a))),
    (Some(a), ..) => Err(CalcitErr::use_string(format!("list `includes?` expected list, list, got: {}", a))),
    (None, ..) => Err(CalcitErr::use_string(format!(
      "list `includes?` expected 2 arguments, got: {:?}",
      xs
    ))),
  }
}

pub fn assoc(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n)), Some(a)) => match f64_to_usize(*n) {
      Ok(idx) => {
        if idx < xs.len() {
          let mut ys = xs.to_owned();
          ys[idx] = a.to_owned();
          Ok(Calcit::List(ys))
        } else {
          Ok(Calcit::Nil)
        }
      }
      Err(e) => Err(CalcitErr::use_string(e)),
    },
    (Some(a), ..) => Err(CalcitErr::use_string(format!("list:assoc expected list, got: {}", a))),
    (None, ..) => Err(CalcitErr::use_string(format!("list:assoc expected 3 arguments, got: {:?}", xs))),
  }
}

pub fn dissoc(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => {
        let ys = &mut xs.to_owned();
        ys.remove(idx);
        Ok(Calcit::List(ys.to_owned()))
      }
      Err(e) => Err(CalcitErr::use_string(format!("dissoc expected number, {}", e))),
    },
    (Some(a), ..) => Err(CalcitErr::use_string(format!("list dissoc expected a list, got: {}", a))),
    (_, _) => Err(CalcitErr::use_string(format!("list dissoc expected 2 arguments, got: {:?}", xs))),
  }
}

pub fn list_to_set(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return Err(CalcitErr::use_string(format!(
      "&list:to-set expected a single argument in list, got {:?}",
      xs
    )));
  }
  match &xs[0] {
    Calcit::List(ys) => {
      let mut zs = im::HashSet::new();
      for y in ys {
        zs.insert(y.to_owned());
      }
      Ok(Calcit::Set(zs))
    }
    a => Err(CalcitErr::use_string(format!("&list:to-set expected a list, got {}", a))),
  }
}

pub fn distinct(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return Err(CalcitErr::use_string(format!(
      "&list:distinct expected a single argument in list, got {:?}",
      xs
    )));
  }
  match &xs[0] {
    Calcit::List(ys) => {
      let mut zs = im::Vector::new();
      for y in ys {
        if !zs.contains(y) {
          zs.push_back(y.to_owned());
        }
      }
      Ok(Calcit::List(zs))
    }
    a => Err(CalcitErr::use_string(format!("&list:distinct expected a list, got {}", a))),
  }
}
