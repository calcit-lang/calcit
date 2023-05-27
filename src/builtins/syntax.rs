//! this file does not cover all syntax instances.
//! syntaxes related to data are maintained the corresponding files
//! Rust has limits on Closures, callbacks need to be handled specifically

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;

use im_ternary_tree::TernaryTreeList;

use crate::builtins;
use crate::builtins::meta::NS_SYMBOL_DICT;
use crate::call_stack::CallStackList;
use crate::primes::{self, CrListWrap, LocatedWarning};
use crate::primes::{gen_core_id, Calcit, CalcitErr, CalcitItems, CalcitScope};
use crate::runner;

pub fn defn(expr: &CalcitItems, scope: &CalcitScope, file_ns: Arc<str>) -> Result<Calcit, CalcitErr> {
  match (expr.get(0), expr.get(1)) {
    (Some(Calcit::Symbol { sym: s, .. }), Some(Calcit::List(xs))) => Ok(Calcit::Fn {
      name: s.to_owned(),
      def_ns: file_ns,
      id: gen_core_id(),
      scope: Arc::new(scope.to_owned()),
      args: Arc::new(get_raw_args(xs)?),
      body: Arc::new(expr.skip(2)?),
    }),
    (Some(a), Some(b)) => CalcitErr::err_str(format!("invalid args type for defn: {a} , {b}")),
    _ => CalcitErr::err_str("inefficient arguments for defn"),
  }
}

pub fn defmacro(expr: &CalcitItems, _scope: &CalcitScope, def_ns: Arc<str>) -> Result<Calcit, CalcitErr> {
  match (expr.get(0), expr.get(1)) {
    (Some(Calcit::Symbol { sym: s, .. }), Some(Calcit::List(xs))) => Ok(Calcit::Macro {
      name: s.to_owned(),
      def_ns,
      id: gen_core_id(),
      args: Arc::new(get_raw_args(xs)?),
      body: Arc::new(expr.skip(2)?),
    }),
    (Some(a), Some(b)) => CalcitErr::err_str(format!("invalid structure for defmacro: {a} {b}")),
    _ => CalcitErr::err_str(format!("invalid structure for defmacro: {}", Calcit::List(expr.to_owned()))),
  }
}

pub fn get_raw_args(args: &CalcitItems) -> Result<Vec<Arc<str>>, String> {
  let mut xs: Vec<Arc<str>> = vec![];
  for item in args {
    if let Calcit::Symbol { sym, .. } = item {
      xs.push(sym.to_owned());
    } else {
      return Err(format!("Unexpected argument: {item}"));
    }
  }
  Ok(xs)
}

pub fn quote(expr: &CalcitItems, _scope: &CalcitScope, _file_ns: Arc<str>) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    Ok(expr[0].to_owned())
  } else {
    CalcitErr::err_str(format!("unexpected data for quote: {expr:?}"))
  }
}

pub fn syntax_if(expr: &CalcitItems, scope: &CalcitScope, file_ns: Arc<str>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  match (expr.get(0), expr.get(1)) {
    _ if expr.len() > 3 => CalcitErr::err_str(format!("too many nodes for if: {expr:?}")),
    (Some(cond), Some(true_branch)) => {
      let cond_value = runner::evaluate_expr(cond, scope, file_ns.to_owned(), call_stack)?;
      match cond_value {
        Calcit::Nil | Calcit::Bool(false) => match expr.get(2) {
          Some(false_branch) => runner::evaluate_expr(false_branch, scope, file_ns, call_stack),
          None => Ok(Calcit::Nil),
        },
        _ => runner::evaluate_expr(true_branch, scope, file_ns, call_stack),
      }
    }
    (None, _) => CalcitErr::err_str(format!("insufficient nodes for if: {expr:?}")),
    _ => CalcitErr::err_str(format!("invalid if form: {expr:?}")),
  }
}

pub fn eval(expr: &CalcitItems, scope: &CalcitScope, file_ns: Arc<str>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    let v = runner::evaluate_expr(&expr[0], scope, file_ns.to_owned(), call_stack)?;
    runner::evaluate_expr(&v, scope, file_ns, call_stack)
  } else {
    CalcitErr::err_str(format!("unexpected data for evaling: {expr:?}"))
  }
}

pub fn syntax_let(expr: &CalcitItems, scope: &CalcitScope, file_ns: Arc<str>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  match expr.get(0) {
    // Some(Calcit::Nil) => runner::evaluate_lines(&expr.drop_left(), scope, file_ns, call_stack),
    Some(Calcit::List(xs)) if xs.is_empty() => runner::evaluate_lines(&expr.drop_left(), scope, file_ns, call_stack),
    Some(Calcit::List(xs)) if xs.len() == 2 => {
      let mut body_scope = scope.to_owned();
      match (&xs[0], &xs[1]) {
        (Calcit::Symbol { sym: s, .. }, ys) => {
          let value = runner::evaluate_expr(ys, scope, file_ns.to_owned(), call_stack)?;
          body_scope.insert(s.to_owned(), value);
        }
        (a, _) => return CalcitErr::err_str(format!("invalid binding name: {a}")),
      }
      runner::evaluate_lines(&expr.drop_left(), &body_scope, file_ns, call_stack)
    }
    Some(Calcit::List(xs)) => CalcitErr::err_str(format!("invalid length: {xs:?}")),
    Some(_) => CalcitErr::err_str(format!("invalid node for &let: {}", CrListWrap(expr.to_owned()))),
    None => CalcitErr::err_str("&let expected a pair or a nil"),
  }
}

// code replaced from `~` and `~@` returns different results
#[derive(Clone, PartialEq, Debug)]
enum SpanResult {
  Single(Calcit),
  Range(Box<CalcitItems>),
}

pub fn quasiquote(expr: &CalcitItems, scope: &CalcitScope, file_ns: Arc<str>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  match expr.get(0) {
    None => CalcitErr::err_str("quasiquote expected a node"),
    Some(code) => {
      match replace_code(code, scope, file_ns, call_stack)? {
        SpanResult::Single(v) => {
          // println!("replace result: {:?}", v);
          Ok(v)
        }
        SpanResult::Range(xs) => CalcitErr::err_str(format!("expected single result from quasiquote, got {xs:?}")),
      }
    }
  }
}

fn replace_code(c: &Calcit, scope: &CalcitScope, file_ns: Arc<str>, call_stack: &CallStackList) -> Result<SpanResult, CalcitErr> {
  if !has_unquote(c) {
    return Ok(SpanResult::Single(c.to_owned()));
  }
  match c {
    Calcit::List(ys) => match (ys.get(0), ys.get(1)) {
      (Some(Calcit::Symbol { sym, .. }), Some(expr)) if &**sym == "~" => {
        let value = runner::evaluate_expr(expr, scope, file_ns, call_stack)?;
        Ok(SpanResult::Single(value))
      }
      (Some(Calcit::Symbol { sym, .. }), Some(expr)) if &**sym == "~@" => {
        let ret = runner::evaluate_expr(expr, scope, file_ns, call_stack)?;
        match ret {
          Calcit::List(zs) => Ok(SpanResult::Range(Box::new(zs))),
          _ => Err(CalcitErr::use_str(format!("unknown result from unquote-slice: {ret}"))),
        }
      }
      (_, _) => {
        let mut ret: TernaryTreeList<Calcit> = TernaryTreeList::Empty;
        for y in ys {
          match replace_code(y, scope, file_ns.to_owned(), call_stack)? {
            SpanResult::Single(z) => ret = ret.push_right(z),
            SpanResult::Range(pieces) => {
              for piece in &*pieces {
                ret = ret.push_right(piece.to_owned());
              }
            }
          }
        }
        Ok(SpanResult::Single(Calcit::List(ret)))
      }
    },
    _ => Ok(SpanResult::Single(c.to_owned())),
  }
}

pub fn has_unquote(xs: &Calcit) -> bool {
  match xs {
    Calcit::List(ys) => {
      for y in ys {
        if has_unquote(y) {
          return true;
        }
      }
      false
    }
    Calcit::Symbol { sym: s, .. } if &**s == "~" || &**s == "~@" => true,
    _ => false,
  }
}

pub fn macroexpand(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: Arc<str>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    let quoted_code = runner::evaluate_expr(&expr[0], scope, file_ns.to_owned(), call_stack)?;

    match &quoted_code {
      Calcit::List(xs) => {
        if xs.is_empty() {
          return Ok(quoted_code);
        }
        let v = runner::evaluate_expr(&xs[0], scope, file_ns, call_stack)?;
        match v {
          Calcit::Macro { def_ns, args, body, .. } => {
            // mutable operation
            let mut rest_nodes = xs.drop_left();
            // println!("macro: {:?} ... {:?}", args, rest_nodes);
            // keep expanding until return value is not a recur
            loop {
              let body_scope = runner::bind_args(&args, &rest_nodes, scope, call_stack)?;
              let v = runner::evaluate_lines(&body, &body_scope, def_ns.to_owned(), call_stack)?;
              match v {
                Calcit::Recur(rest_code) => {
                  rest_nodes = rest_code.to_owned();
                }
                _ => return Ok(v),
              }
            }
          }
          _ => Ok(quoted_code),
        }
      }
      a => Ok(a.to_owned()),
    }
  } else {
    CalcitErr::err_str(format!("macroexpand expected excaclty 1 argument, got: {expr:?}"))
  }
}

pub fn macroexpand_1(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: Arc<str>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    let quoted_code = runner::evaluate_expr(&expr[0], scope, file_ns.to_owned(), call_stack)?;
    // println!("quoted: {}", quoted_code);
    match &quoted_code {
      Calcit::List(xs) => {
        if xs.is_empty() {
          return Ok(quoted_code);
        }
        let v = runner::evaluate_expr(&xs[0], scope, file_ns, call_stack)?;
        match v {
          Calcit::Macro { def_ns, args, body, .. } => {
            let body_scope = runner::bind_args(&args, &xs.drop_left(), scope, call_stack)?;
            runner::evaluate_lines(&body, &body_scope, def_ns, call_stack)
          }
          _ => Ok(quoted_code),
        }
      }
      a => Ok(a.to_owned()),
    }
  } else {
    CalcitErr::err_str(format!("macroexpand expected excaclty 1 argument, got: {expr:?}"))
  }
}

pub fn macroexpand_all(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: Arc<str>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    let quoted_code = runner::evaluate_expr(&expr[0], scope, file_ns.to_owned(), call_stack)?;

    match &quoted_code {
      Calcit::List(xs) => {
        if xs.is_empty() {
          return Ok(quoted_code);
        }
        let v = runner::evaluate_expr(&xs[0], scope, file_ns.to_owned(), call_stack)?;
        match v {
          Calcit::Macro { def_ns, args, body, .. } => {
            // mutable operation
            let mut rest_nodes = xs.drop_left();
            let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
            // println!("macro: {:?} ... {:?}", args, rest_nodes);
            // keep expanding until return value is not a recur
            loop {
              let body_scope = runner::bind_args(&args, &rest_nodes, scope, call_stack)?;
              let v = runner::evaluate_lines(&body, &body_scope, def_ns.to_owned(), call_stack)?;
              match v {
                Calcit::Recur(rest_code) => {
                  rest_nodes = rest_code.to_owned();
                }
                _ => {
                  let (resolved, _v) = runner::preprocess::preprocess_expr(&v, &HashSet::new(), file_ns, check_warnings, call_stack)?;
                  let warnings = check_warnings.borrow();
                  LocatedWarning::print_list(&warnings);

                  return Ok(resolved);
                }
              }
            }
          }
          _ => {
            let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
            let (resolved, _v) =
              runner::preprocess::preprocess_expr(&quoted_code, &HashSet::new(), file_ns, check_warnings, call_stack)?;
            LocatedWarning::print_list(&check_warnings.borrow());
            Ok(resolved)
          }
        }
      }
      a => Ok(a.to_owned()),
    }
  } else {
    CalcitErr::err_str(format!("macroexpand expected excaclty 1 argument, got: {expr:?}"))
  }
}

pub fn call_try(expr: &CalcitItems, scope: &CalcitScope, file_ns: Arc<str>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if expr.len() == 2 {
    let xs = runner::evaluate_expr(&expr[0], scope, file_ns.to_owned(), call_stack);

    match &xs {
      // dirty since only functions being call directly then we become fast
      Ok(v) => Ok(v.to_owned()),
      Err(failure) => {
        let f = runner::evaluate_expr(&expr[1], scope, file_ns, call_stack)?;
        let err_data = Calcit::Str(failure.msg.to_owned().into());
        match f {
          Calcit::Fn {
            def_ns, scope, args, body, ..
          } => {
            let values = TernaryTreeList::from(&[err_data]);
            runner::run_fn(&values, &scope, &args, &body, def_ns, call_stack)
          }
          Calcit::Proc(proc) => builtins::handle_proc(proc, &TernaryTreeList::from(&[err_data]), call_stack),
          a => CalcitErr::err_str(format!("try expected a function handler, got: {a}")),
        }
      }
    }
  } else {
    CalcitErr::err_str(format!("try expected 2 arguments, got: {expr:?}"))
  }
}

pub fn gensym(xs: &CalcitItems, _scope: &CalcitScope, file_ns: Arc<str>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let n = {
    let mut ns_sym_dict = NS_SYMBOL_DICT.lock().expect("open symbol dict");
    // println!("calling in ns: {}", file_ns);
    if let Some(n) = ns_sym_dict.get_mut(&file_ns) {
      let v = n.to_owned();
      *n += 1;
      v
    } else {
      ns_sym_dict.insert(file_ns.to_owned(), 2);
      1
    }
  };

  let s = if xs.is_empty() {
    let mut chunk = String::from("G__");
    chunk.push_str(&n.to_string());
    chunk
  } else {
    match &xs[0] {
      Calcit::Str(s) | Calcit::Symbol { sym: s, .. } => {
        let mut chunk = (**s).to_string();
        chunk.push('_');
        chunk.push('_');
        chunk.push_str(&n.to_string());
        chunk
      }
      Calcit::Tag(k) => {
        let mut chunk = k.to_string();
        chunk.push('_');
        chunk.push('_');
        chunk.push_str(&n.to_string());
        chunk
      }
      a => return CalcitErr::err_str(format!("gensym expected a string, but got: {a}")),
    }
  };
  Ok(Calcit::Symbol {
    sym: s.into(),
    ns: file_ns,
    at_def: primes::GENERATED_DEF.into(),
    resolved: None,
    location: None,
  })
}
