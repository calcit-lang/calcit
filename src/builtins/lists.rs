use core::cmp::Ordering;

use crate::primes::{Calcit, CalcitItems, CalcitScope};
use crate::util::number::f64_to_usize;

use crate::builtins;
use crate::program::ProgramCodeData;
use crate::runner;

pub fn new_list(xs: &CalcitItems) -> Result<Calcit, String> {
  Ok(Calcit::List(xs.clone()))
}

pub fn empty_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Bool(true)),
    Some(Calcit::Tuple(..)) => Ok(Calcit::Bool(false)),
    Some(Calcit::List(ys)) => Ok(Calcit::Bool(ys.is_empty())),
    Some(Calcit::Map(ys)) => Ok(Calcit::Bool(ys.is_empty())),
    Some(Calcit::Set(ys)) => Ok(Calcit::Bool(ys.is_empty())),
    Some(Calcit::Str(s)) => Ok(Calcit::Bool(s.is_empty())),
    Some(a) => Err(format!("empty? expected some seq, got: {}", a)),
    None => Err(String::from("empty? expected 1 argument")),
  }
}

pub fn count(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Number(0.0)),
    Some(Calcit::Tuple(..)) => Ok(Calcit::Number(2.0)),
    Some(Calcit::List(ys)) => Ok(Calcit::Number(ys.len() as f64)),
    Some(Calcit::Map(ys)) => Ok(Calcit::Number(ys.len() as f64)),
    Some(Calcit::Set(ys)) => Ok(Calcit::Number(ys.len() as f64)),
    Some(Calcit::Str(s)) => Ok(Calcit::Number(s.chars().count() as f64)),
    Some(Calcit::Record(_name, fields, _)) => Ok(Calcit::Number(fields.len() as f64)),
    Some(a) => Err(format!("count expected some seq, got: {}", a)),
    None => Err(String::from("count expected 1 argument")),
  }
}

pub fn nth(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Nil), Some(Calcit::Number(_))) => Ok(Calcit::Nil),
    (Some(Calcit::Tuple(a, b)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(0) => Ok((**a).clone()),
      Ok(1) => Ok((**b).to_owned()),
      Ok(m) => Err(format!("Tuple only got 2 elements, trying to index with {}", m)),
      Err(e) => Err(format!("nth expect usize, {}", e)),
    },
    (Some(Calcit::List(ys)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => match ys.get(idx) {
        Some(v) => Ok(v.clone()),
        None => Ok(Calcit::Nil),
      },
      Err(e) => Err(format!("nth expect usize, {}", e)),
    },
    (Some(Calcit::Str(s)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => match s.chars().nth(idx) {
        Some(v) => Ok(Calcit::Str(v.to_string())),
        None => Ok(Calcit::Nil),
      },
      Err(e) => Err(format!("nth expect usize, {}", e)),
    },
    (Some(Calcit::Record(_name, fields, values)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => {
        if idx < fields.len() {
          Ok(Calcit::List(im::vector![
            Calcit::Keyword(fields[idx].clone()),
            values[idx].clone()
          ]))
        } else {
          Ok(Calcit::Nil)
        }
      }
      Err(e) => Err(format!("nth expect usize, {}", e)),
    },
    (Some(_), None) => Err(format!("nth expected a ordered seq and index, got: {:?}", xs)),
    (None, Some(_)) => Err(format!("nth expected a ordered seq and index, got: {:?}", xs)),
    (_, _) => Err(format!("nth expected 2 argument, got: {:?}", xs)),
  }
}

pub fn slice(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(ys)), Some(Calcit::Number(from))) => {
      let to_idx = match xs.get(2) {
        Some(Calcit::Number(to)) => {
          let idx: usize = unsafe { to.to_int_unchecked() };
          idx
        }
        Some(a) => return Err(format!("slice expected number index, got: {}", a)),
        None => ys.len(),
      };
      let from_idx: usize = unsafe { from.to_int_unchecked() };
      Ok(Calcit::List(ys.clone().slice(from_idx..to_idx)))
    }
    (Some(Calcit::List(_)), Some(a)) => Err(format!("slice expected index number, got: {}", a)),
    (Some(Calcit::List(_)), None) => Err(String::from("slice expected index numbers")),
    (_, _) => Err(String::from("slice expected 2~3 arguments")),
  }
}

pub fn append(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(ys)), Some(a)) => {
      let mut zs = ys.clone();
      zs.push_back(a.clone());
      Ok(Calcit::List(zs))
    }
    (Some(a), _) => Err(format!("append expected list, got: {}", a)),
    (None, _) => Err(String::from("append expected 2 arguments, got nothing")),
  }
}

pub fn prepend(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(ys)), Some(a)) => {
      let mut zs = ys.clone();
      zs.push_front(a.clone());
      Ok(Calcit::List(zs))
    }
    (Some(a), _) => Err(format!("prepend expected list, got: {}", a)),
    (None, _) => Err(String::from("prepend expected 2 arguments, got nothing")),
  }
}

pub fn rest(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Nil),
    Some(Calcit::List(ys)) => {
      if ys.is_empty() {
        Ok(Calcit::Nil)
      } else {
        let mut zs = ys.clone();
        zs.pop_front();
        Ok(Calcit::List(zs))
      }
    }
    Some(Calcit::Set(ys)) => match ys.iter().next() {
      Some(y0) => {
        let mut zs = ys.clone();
        zs.remove(y0);
        Ok(Calcit::Set(zs))
      }
      None => Ok(Calcit::Nil),
    },
    Some(Calcit::Map(ys)) => match ys.keys().next() {
      Some(k0) => {
        let mut zs = ys.clone();
        zs.remove(k0);
        Ok(Calcit::Map(zs))
      }
      None => Ok(Calcit::Nil),
    },
    Some(Calcit::Str(s)) => {
      let mut buffer = String::from("");
      let mut is_first = true;
      for c in s.chars() {
        if is_first {
          is_first = false;
          continue;
        }
        buffer.push(c)
      }
      Ok(Calcit::Str(buffer.to_owned()))
    }
    Some(a) => Err(format!("rest expected a list, got: {}", a)),
    None => Err(String::from("rest expected 1 argument")),
  }
}

pub fn butlast(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Nil),
    Some(Calcit::List(ys)) => {
      if ys.is_empty() {
        Ok(Calcit::Nil)
      } else {
        let mut zs = ys.clone();
        zs.pop_back();
        Ok(Calcit::List(zs))
      }
    }
    Some(a) => Err(format!("butlast expected a list, got: {}", a)),
    None => Err(String::from("butlast expected 1 argument")),
  }
}

pub fn concat(xs: &CalcitItems) -> Result<Calcit, String> {
  let mut ys: CalcitItems = im::vector![];
  for x in xs {
    if let Calcit::List(zs) = x {
      for z in zs {
        ys.push_back(z.clone());
      }
    } else {
      return Err(format!("concat expects list arguments, got: {}", x));
    }
  }
  Ok(Calcit::List(ys))
}

pub fn range(xs: &CalcitItems) -> Result<Calcit, String> {
  let (base, bound) = match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(bound)), None) => (0.0, *bound),
    (Some(Calcit::Number(base)), Some(Calcit::Number(bound))) => (*base, *bound),
    (Some(a), Some(b)) => return Err(format!("range expected 2 numbers, but got: {} {}", a, b)),
    (_, _) => return Err(format!("invalid arguments for range: {:?}", xs)),
  };

  let step = match xs.get(2) {
    Some(Calcit::Number(n)) => *n,
    Some(a) => return Err(format!("range expected numbers, but got: {}", a)),
    None => 1.0,
  };

  if (bound - base).abs() < f64::EPSILON {
    return Ok(Calcit::List(im::vector![]));
  }

  if step == 0.0 || (bound > base && step < 0.0) || (bound < base && step > 0.0) {
    return Err(String::from("range cannot construct list with step 0"));
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

pub fn reverse(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Nil),
    Some(Calcit::List(ys)) => {
      let mut zs: CalcitItems = im::vector![];
      for y in ys {
        zs.push_front(y.clone());
      }
      Ok(Calcit::List(zs))
    }
    Some(a) => Err(format!("butlast expected a list, got: {}", a)),
    None => Err(String::from("butlast expected 1 argument")),
  }
}

/// foldl using syntax for performance, it's supposed to be a function
pub fn foldl(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, String> {
  if expr.len() == 3 {
    let xs = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;
    let acc = runner::evaluate_expr(&expr[1], scope, file_ns, program_code)?;
    let f = runner::evaluate_expr(&expr[2], scope, file_ns, program_code)?;
    match (&xs, &f) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        let mut ret = acc;
        for x in xs {
          let values = im::vector![ret, x.clone()];
          ret = runner::run_fn(&values, &def_scope, args, body, def_ns, program_code)?;
        }
        Ok(ret)
      }
      (Calcit::List(xs), Calcit::Proc(proc)) => {
        let mut ret = acc;
        for x in xs {
          // println!("foldl args, {} {}", ret, x.clone());
          ret = builtins::handle_proc(&proc, &im::vector![ret, x.clone()])?;
        }
        Ok(ret)
      }
      // also handles set
      (Calcit::Set(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        let mut ret = acc;
        for x in xs {
          let values = im::vector![ret, x.clone()];
          ret = runner::run_fn(&values, &def_scope, args, body, def_ns, program_code)?;
        }
        Ok(ret)
      }
      (Calcit::Set(xs), Calcit::Proc(proc)) => {
        let mut ret = acc;
        for x in xs {
          // println!("foldl args, {} {}", ret, x.clone());
          ret = builtins::handle_proc(&proc, &im::vector![ret, x.clone()])?;
        }
        Ok(ret)
      }
      // also handles map
      (Calcit::Map(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        let mut ret = acc;
        for (k, x) in xs {
          let values = im::vector![ret, Calcit::List(im::vector![k.to_owned(), x.to_owned()])];
          ret = runner::run_fn(&values, &def_scope, args, body, def_ns, program_code)?;
        }
        Ok(ret)
      }
      (Calcit::Map(xs), Calcit::Proc(proc)) => {
        let mut ret = acc;
        for (k, x) in xs {
          // println!("foldl args, {} {}", ret, x.clone());
          ret = builtins::handle_proc(
            &proc,
            &im::vector![ret, Calcit::List(im::vector![k.to_owned(), x.to_owned()])],
          )?;
        }
        Ok(ret)
      }

      (_, _) => Err(format!("foldl expected list and function, got: {} {}", xs, f)),
    }
  } else {
    Err(format!("foldl expected 3 arguments, got: {:?}", expr))
  }
}

/// foldl-shortcut using syntax for performance, it's supposed to be a function
/// by returning `[bool, acc]`, bool indicates where performace a shortcut return
pub fn foldl_shortcut(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, String> {
  if expr.len() == 4 {
    let xs = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;
    let acc = runner::evaluate_expr(&expr[1], scope, file_ns, program_code)?;
    let default_value = runner::evaluate_expr(&expr[2], scope, file_ns, program_code)?;
    let f = runner::evaluate_expr(&expr[3], scope, file_ns, program_code)?;
    match (&xs, &f) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        let mut state = acc;
        for x in xs {
          let values = im::vector![state, x.clone()];
          let pair = runner::run_fn(&values, &def_scope, args, body, def_ns, program_code)?;
          match pair {
            Calcit::List(ys) if ys.len() == 2 => match &ys[0] {
              Calcit::Bool(b) => {
                if *b {
                  return Ok(ys[1].to_owned());
                } else {
                  state = ys[1].to_owned()
                }
              }
              a => return Err(format!("return value in foldl-shortcut should be a bool, got: {}", a)),
            },
            _ => {
              return Err(format!(
                "return value for foldl-shortcut should be `[bool, acc]`, got: {}",
                pair
              ))
            }
          }
        }
        Ok(default_value)
      }
      // almost identical body, escept for the type
      (Calcit::Set(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        let mut state = acc;
        for x in xs {
          let values = im::vector![state, x.clone()];
          let pair = runner::run_fn(&values, &def_scope, args, body, def_ns, program_code)?;
          match pair {
            Calcit::List(ys) if ys.len() == 2 => match &ys[0] {
              Calcit::Bool(b) => {
                if *b {
                  return Ok(ys[1].to_owned());
                } else {
                  state = ys[1].to_owned()
                }
              }
              a => return Err(format!("return value in foldl-shortcut should be a bool, got: {}", a)),
            },
            _ => {
              return Err(format!(
                "return value for foldl-shortcut should be `[bool, acc]`, got: {}",
                pair
              ))
            }
          }
        }
        Ok(default_value)
      }
      // almost identical body, escept for the type
      (Calcit::Map(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        let mut state = acc;
        for (k, x) in xs {
          let values = im::vector![state, Calcit::List(im::vector![k.to_owned(), x.to_owned()])];
          let pair = runner::run_fn(&values, &def_scope, args, body, def_ns, program_code)?;
          match pair {
            Calcit::List(ys) if ys.len() == 2 => match &ys[0] {
              Calcit::Bool(b) => {
                if *b {
                  return Ok(ys[1].to_owned());
                } else {
                  state = ys[1].to_owned()
                }
              }
              a => return Err(format!("return value in foldl-shortcut should be a bool, got: {}", a)),
            },
            _ => {
              return Err(format!(
                "return value for foldl-shortcut should be `[bool, acc]`, got: {}",
                pair
              ))
            }
          }
        }
        Ok(default_value)
      }

      (_, _) => Err(format!("foldl-shortcut expected list... and fn, got: {} {}", xs, f)),
    }
  } else {
    Err(format!(
      "foldl-shortcut expected 4 arguments list,state,default,fn, got: {:?}",
      expr
    ))
  }
}

// TODO as SYNTAX at current, not supposed to be a syntax
pub fn sort(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, String> {
  if expr.len() == 2 {
    let xs = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;
    let f = runner::evaluate_expr(&expr[1], scope, file_ns, program_code)?;
    match (&xs, &f) {
      // dirty since only functions being call directly then we become fast
      (Calcit::List(xs), Calcit::Fn(_, def_ns, _, def_scope, args, body)) => {
        let mut ret = xs.clone();
        ret.sort_by(|a, b| {
          let values = im::vector![a.clone(), b.clone()];
          let v = runner::run_fn(&values, &def_scope, args, body, def_ns, program_code);
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
        let mut ret = xs.clone();
        ret.sort_by(|a, b| {
          let values = im::vector![a.clone(), b.clone()];
          let v = builtins::handle_proc(&proc, &values);
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

      (_, _) => Err(format!("sort expected list and function, got: {} {}", xs, f)),
    }
  } else {
    Err(format!("sort expected 2 arguments, got: {:?}", expr))
  }
}

/// use builtin function since sets need to be handled specifically
pub fn first(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Nil),
    Some(Calcit::List(ys)) => {
      if ys.is_empty() {
        Ok(Calcit::Nil)
      } else {
        Ok(ys[0].clone())
      }
    }
    Some(Calcit::Set(ys)) => match ys.iter().next() {
      // TODO first element of a set.. need to be more sure...
      Some(v) => Ok(v.clone()),
      None => Ok(Calcit::Nil),
    },
    Some(Calcit::Map(ys)) => match ys.iter().next() {
      // TODO order may not be stable enough
      Some((k, v)) => Ok(Calcit::List(im::vector![k.to_owned(), v.to_owned()])),
      None => Ok(Calcit::Nil),
    },
    Some(Calcit::Str(s)) => match s.chars().next() {
      Some(c) => Ok(Calcit::Str(c.to_string())),
      None => Ok(Calcit::Nil),
    },
    Some(a) => Err(format!("first expected a list, got: {}", a)),
    None => Err(String::from("first expected 1 argument")),
  }
}

// real implementation relies of ternary-tree
pub fn assoc_before(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n)), Some(a)) => match f64_to_usize(*n) {
      Ok(idx) => {
        let mut ys = xs.to_owned();
        ys.insert(idx, a.to_owned());
        Ok(Calcit::List(ys))
      }
      Err(e) => Err(format!("assoc-before expect usize, {}", e)),
    },
    (Some(a), Some(b), Some(c)) => Err(format!("assoc-before expected list and index, got: {} {} {}", a, b, c)),
    (a, b, c) => Err(format!("invalid arguments to assoc-before: {:?} {:?} {:?}", a, b, c)),
  }
}

pub fn assoc_after(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n)), Some(a)) => match f64_to_usize(*n) {
      Ok(idx) => {
        let mut ys = xs.to_owned();
        ys.insert(idx + 1, a.to_owned());
        Ok(Calcit::List(ys))
      }
      Err(e) => Err(format!("assoc-after expect usize, {}", e)),
    },
    (Some(a), Some(b), Some(c)) => Err(format!("assoc-after expected list and index, got: {} {} {}", a, b, c)),
    (a, b, c) => Err(format!("invalid arguments to assoc-after: {:?} {:?} {:?}", a, b, c)),
  }
}
