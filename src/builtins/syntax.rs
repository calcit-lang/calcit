//! this file does not cover all syntax instances.
//! syntaxes related to data are maintained the corresponding files
//! Rust has limits on Closures, callbacks need to be handled specifically

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;

use crate::builtins;
use crate::builtins::meta::NS_SYMBOL_DICT;
use crate::calcit::{
  self, CalcitArgLabel, CalcitFn, CalcitFnArgs, CalcitList, CalcitLocal, CalcitMacro, CalcitSymbolInfo, CalcitSyntax, LocatedWarning,
};
use crate::calcit::{gen_core_id, Calcit, CalcitErr, CalcitScope};
use crate::call_stack::CallStackList;
use crate::runner::{self, call_expr, evaluate_expr};

pub fn defn(expr: &CalcitList, scope: &CalcitScope, file_ns: &str) -> Result<Calcit, CalcitErr> {
  match (expr.get_inner(0), expr.get_inner(1)) {
    (Some(Calcit::Symbol { sym: s, .. }), Some(Calcit::List(xs))) => Ok(Calcit::Fn {
      id: gen_core_id(),
      info: Arc::new(CalcitFn {
        name: s.to_owned(),
        def_ns: Arc::from(file_ns),
        scope: Arc::new(scope.to_owned()),
        args: Arc::new(get_raw_args_fn(xs)?),
        body: Arc::new(expr.skip(2)?.into()),
      }),
    }),
    (Some(a), Some(b)) => CalcitErr::err_str(format!("invalid args type for defn: {a} , {b}")),
    _ => CalcitErr::err_str("inefficient arguments for defn"),
  }
}

pub fn defmacro(expr: &CalcitList, _scope: &CalcitScope, def_ns: &str) -> Result<Calcit, CalcitErr> {
  match (expr.get_inner(0), expr.get_inner(1)) {
    (Some(Calcit::Symbol { sym: s, .. }), Some(Calcit::List(xs))) => Ok(Calcit::Macro {
      id: gen_core_id(),
      info: Arc::new(CalcitMacro {
        name: s.to_owned(),
        def_ns: Arc::from(def_ns),
        args: Arc::new(get_raw_args(xs)?),
        body: Arc::new(expr.skip(2)?.into()),
      }),
    }),
    (Some(a), Some(b)) => CalcitErr::err_str(format!("invalid structure for defmacro: {a} {b}")),
    _ => CalcitErr::err_str(format!("invalid structure for defmacro: {}", Calcit::from(expr.to_owned()))),
  }
}

pub fn get_raw_args(args: &CalcitList) -> Result<Vec<CalcitArgLabel>, String> {
  let mut xs: Vec<CalcitArgLabel> = vec![];
  for item in args {
    match item {
      Calcit::Local(CalcitLocal { idx, .. }) => {
        xs.push(CalcitArgLabel::Idx(*idx));
      }
      Calcit::Syntax(CalcitSyntax::ArgOptional, _) => {
        xs.push(CalcitArgLabel::OptionalMark);
      }
      Calcit::Syntax(CalcitSyntax::ArgSpread, _) => {
        xs.push(CalcitArgLabel::RestMark);
      }
      Calcit::Symbol { sym, .. } => {
        let idx = CalcitLocal::track_sym(sym);
        // during macro processing, we still git symbol
        xs.push(CalcitArgLabel::Idx(idx));
        // return Err(format!("Unexpected argument label: {item}"));
      }
      _ => {
        return Err(format!("raw args unexpected argument: {item}"));
      }
    }
  }
  // println!("Making macro args: {:?} from {:?}", xs, args);
  Ok(xs)
}

pub fn get_raw_args_fn(args: &CalcitList) -> Result<CalcitFnArgs, String> {
  let mut xs: Vec<CalcitArgLabel> = vec![];
  let mut has_mark = false;
  for item in args {
    match item {
      Calcit::Local(CalcitLocal { idx, .. }) => {
        xs.push(CalcitArgLabel::Idx(*idx));
      }
      Calcit::Syntax(CalcitSyntax::ArgSpread, _) => {
        xs.push(CalcitArgLabel::RestMark);
        has_mark = true;
      }
      Calcit::Syntax(CalcitSyntax::ArgOptional, _) => {
        xs.push(CalcitArgLabel::OptionalMark);
        has_mark = true;
      }
      Calcit::Symbol { sym, .. } => {
        let idx = CalcitLocal::track_sym(sym);
        // during macro processing, we still git symbol
        xs.push(CalcitArgLabel::Idx(idx));
      }
      _ => {
        return Err(format!("raw args fn unexpected argument: {item:?}"));
      }
    }
  }
  if has_mark {
    Ok(CalcitFnArgs::MarkedArgs(xs))
  } else {
    let mut ys: Vec<u16> = vec![];
    for x in &xs {
      match x {
        CalcitArgLabel::Idx(idx) => {
          ys.push(*idx);
        }
        _ => return Err(format!("Unexpected argument label: {x}")),
      }
    }
    Ok(CalcitFnArgs::Args(ys))
  }
}

pub fn quote(expr: &CalcitList, _scope: &CalcitScope, _file_ns: &str) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    Ok(expr[0].to_owned())
  } else {
    CalcitErr::err_nodes("unexpected data for quote, got:", &expr.to_vec())
  }
}

pub fn syntax_if(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let l = expr.len();
  if l > 3 {
    return CalcitErr::err_nodes("too many nodes for if, got:", &expr.to_vec());
  }
  if l < 2 {
    return CalcitErr::err_nodes("insufficient nodes for if, got:", &expr.to_vec());
  }
  let cond = &expr[0];
  let true_branch = &expr[1];

  let cond_value = runner::evaluate_expr(cond, scope, file_ns, call_stack)?;
  match cond_value {
    Calcit::Nil | Calcit::Bool(false) => match expr.get(2) {
      Some(false_branch) => runner::evaluate_expr(false_branch, scope, file_ns, call_stack),
      None => Ok(Calcit::Nil),
    },
    _ => runner::evaluate_expr(true_branch, scope, file_ns, call_stack),
  }
}

pub fn eval(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    let v = runner::evaluate_expr(&expr[0], scope, file_ns, call_stack)?;
    runner::evaluate_expr(&v, scope, file_ns, call_stack)
  } else {
    CalcitErr::err_nodes("unexpected data for evaling, got:", &expr.to_vec())
  }
}

pub fn syntax_let(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  match expr.get_inner(0) {
    // Some(Calcit::Nil) => runner::evaluate_lines(&expr.drop_left(), scope, file_ns, call_stack),
    Some(Calcit::List(xs)) if xs.is_empty() => runner::evaluate_lines(&expr.drop_left().into(), scope, file_ns, call_stack),
    Some(Calcit::List(xs)) if xs.len() == 2 => {
      let mut body_scope = scope.to_owned();
      match (&xs[0], &xs[1]) {
        (Calcit::Local(CalcitLocal { idx, .. }), ys) => {
          let value = runner::evaluate_expr(ys, scope, file_ns, call_stack)?;
          body_scope.insert_mut(*idx, value);
        }
        (Calcit::Symbol { sym: s, .. }, ys) => {
          println!("[Warn] slow path of {s}, prefer local");
          let value = runner::evaluate_expr(ys, scope, file_ns, call_stack)?;
          let idx = CalcitLocal::track_sym(s);
          body_scope.insert_mut(idx, value);
        }
        (a, _) => return CalcitErr::err_str(format!("invalid binding name: {a}")),
      }
      runner::evaluate_lines(&expr.drop_left().into(), &body_scope, file_ns, call_stack)
    }
    Some(Calcit::List(xs)) => CalcitErr::err_nodes("invalid length for &let , got:", &(xs.0.to_vec())),
    Some(_) => CalcitErr::err_str(format!("invalid node for &let: {}", expr.to_owned())),
    None => CalcitErr::err_str("&let expected a pair or a nil"),
  }
}

// code replaced from `~` and `~@` returns different results
#[derive(Clone, PartialEq, Debug)]
enum SpanResult {
  Single(Calcit),
  Range(Arc<CalcitList>),
}

pub fn quasiquote(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  match expr.get(0) {
    None => CalcitErr::err_str("quasiquote expected a node"),
    Some(code) => {
      match replace_code(code, scope, file_ns, call_stack)? {
        SpanResult::Single(v) => {
          // println!("replace result: {:?}", v);
          Ok(v)
        }
        SpanResult::Range(xs) => CalcitErr::err_nodes("expected single result from quasiquote, got:", &xs.0.to_vec()),
      }
    }
  }
}

fn replace_code(c: &Calcit, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<SpanResult, CalcitErr> {
  if !has_unquote(c) {
    return Ok(SpanResult::Single(c.to_owned()));
  }
  match c {
    Calcit::List(ys) => match (ys.get_inner(0), ys.get_inner(1)) {
      (Some(Calcit::Syntax(CalcitSyntax::MacroInterpolate, _)), Some(expr)) => {
        let value = runner::evaluate_expr(expr, scope, file_ns, call_stack)?;
        Ok(SpanResult::Single(value))
      }
      (Some(Calcit::Syntax(CalcitSyntax::MacroInterpolateSpread, _)), Some(expr)) => {
        let ret = runner::evaluate_expr(expr, scope, file_ns, call_stack)?;
        match ret {
          Calcit::List(zs) => Ok(SpanResult::Range(zs.to_owned())),
          _ => Err(CalcitErr::use_str(format!("unknown result from unquote-slice: {ret}"))),
        }
      }
      (_, _) => {
        let mut ret = CalcitList::new_inner();
        for y in &**ys {
          match replace_code(y, scope, file_ns, call_stack)? {
            SpanResult::Single(z) => ret = ret.push_right(z),
            SpanResult::Range(pieces) => {
              for piece in &*pieces {
                ret = ret.push_right(piece.to_owned());
              }
            }
          }
        }
        Ok(SpanResult::Single(Calcit::from(CalcitList::from(ret))))
      }
    },
    _ => Ok(SpanResult::Single(c.to_owned())),
  }
}

pub fn has_unquote(xs: &Calcit) -> bool {
  match xs {
    Calcit::List(ys) => {
      for y in &ys.0 {
        if has_unquote(y) {
          return true;
        }
      }
      false
    }
    Calcit::Syntax(CalcitSyntax::MacroInterpolate, _) => true,
    Calcit::Syntax(CalcitSyntax::MacroInterpolateSpread, _) => true,
    _ => false,
  }
}

pub fn macroexpand(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    let quoted_code = runner::evaluate_expr(&expr[0], scope, file_ns, call_stack)?;

    match &quoted_code {
      Calcit::List(xs) => {
        if xs.is_empty() {
          return Ok(quoted_code);
        }
        let v = runner::evaluate_expr(&xs[0], scope, file_ns, call_stack)?;
        match v {
          Calcit::Macro { info, .. } => {
            // mutable operation
            let mut rest_nodes: Vec<Calcit> = xs.drop_left().to_vec();
            let mut body_scope = scope.to_owned();
            // println!("macro: {:?} ... {:?}", args, rest_nodes);
            // keep expanding until return value is not a recur
            loop {
              runner::bind_marked_args(&mut body_scope, &info.args, &rest_nodes.to_vec(), call_stack)?;
              let v = runner::evaluate_lines(&info.body, &body_scope, &info.def_ns, call_stack)?;
              match v {
                Calcit::Recur(rest_code) => {
                  rest_nodes = (*rest_code).to_owned();
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
    CalcitErr::err_nodes("macroexpand expected excaclty 1 argument, got:", &expr.to_vec())
  }
}

pub fn macroexpand_1(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    let quoted_code = runner::evaluate_expr(&expr[0], scope, file_ns, call_stack)?;
    // println!("quoted: {}", quoted_code);
    match &quoted_code {
      Calcit::List(xs) => {
        if xs.is_empty() {
          return Ok(quoted_code);
        }
        let v = runner::evaluate_expr(&xs[0], scope, file_ns, call_stack)?;
        match v {
          Calcit::Macro { info, .. } => {
            let mut body_scope = scope.to_owned();
            runner::bind_marked_args(&mut body_scope, &info.args, &xs.drop_left().to_vec(), call_stack)?;
            runner::evaluate_lines(&info.body, &body_scope, &info.def_ns, call_stack)
          }
          _ => Ok(quoted_code),
        }
      }
      a => Ok(a.to_owned()),
    }
  } else {
    CalcitErr::err_nodes("macroexpand expected excaclty 1 argument, got:", &expr.to_vec())
  }
}

pub fn macroexpand_all(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    let quoted_code = runner::evaluate_expr(&expr[0], scope, file_ns, call_stack)?;

    match &quoted_code {
      Calcit::List(xs) => {
        if xs.is_empty() {
          return Ok(quoted_code);
        }
        let v = runner::evaluate_expr(&xs[0], scope, file_ns, call_stack)?;
        match v {
          Calcit::Macro { info, .. } => {
            // mutable operation
            let mut rest_nodes: Vec<Calcit> = xs.drop_left().to_vec();
            let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
            let mut body_scope = scope.to_owned();
            // println!("macro: {:?} ... {:?}", args, rest_nodes);
            // keep expanding until return value is not a recur
            loop {
              runner::bind_marked_args(&mut body_scope, &info.args, &rest_nodes, call_stack)?;
              let v = runner::evaluate_lines(&info.body, &body_scope, &info.def_ns, call_stack)?;
              match v {
                Calcit::Recur(rest_code) => {
                  rest_nodes = (*rest_code).to_vec();
                }
                _ => {
                  let resolved = runner::preprocess::preprocess_expr(&v, &HashSet::new(), file_ns, check_warnings, call_stack)?;
                  let warnings = check_warnings.borrow();
                  LocatedWarning::print_list(&warnings);

                  return Ok(resolved);
                }
              }
            }
          }
          _ => {
            let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
            let resolved = runner::preprocess::preprocess_expr(&quoted_code, &HashSet::new(), file_ns, check_warnings, call_stack)?;
            LocatedWarning::print_list(&check_warnings.borrow());
            Ok(resolved)
          }
        }
      }
      a => Ok(a.to_owned()),
    }
  } else {
    CalcitErr::err_nodes("macroexpand expected excaclty 1 argument, got:", &expr.to_vec())
  }
}

/// inserted automatically when `&` syntax is recognized in calling
pub fn call_spread(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if expr.len() < 3 {
    return CalcitErr::err_nodes("call-spread expected at least 3 arguments, got:", &expr.to_vec());
  }

  let x = &expr[0];

  if x.is_expr_evaluated() {
    call_expr(x, expr, scope, file_ns, call_stack, true)
  } else {
    let v = evaluate_expr(x, scope, file_ns, call_stack)?;
    call_expr(&v, expr, scope, file_ns, call_stack, true)
  }
}

pub fn call_try(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if expr.len() == 2 {
    let xs = runner::evaluate_expr(&expr[0], scope, file_ns, call_stack);

    match &xs {
      // dirty since only functions being call directly then we become fast
      Ok(v) => Ok(v.to_owned()),
      Err(failure) => {
        let f = runner::evaluate_expr(&expr[1], scope, file_ns, call_stack)?;
        let err_data = Calcit::Str(failure.msg.to_owned().into());
        match f {
          Calcit::Fn { info, .. } => runner::run_fn(&[err_data], &info, call_stack),
          Calcit::Proc(proc) => builtins::handle_proc(proc, &[err_data], call_stack),
          a => CalcitErr::err_str(format!("try expected a function handler, got: {a}")),
        }
      }
    }
  } else {
    CalcitErr::err_nodes("try expected 2 arguments, got:", &expr.to_vec())
  }
}

pub fn gensym(xs: &CalcitList, _scope: &CalcitScope, file_ns: &str, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let n = {
    let mut ns_sym_dict = NS_SYMBOL_DICT.lock().expect("open symbol dict");
    // println!("calling in ns: {}", file_ns);
    if let Some(n) = ns_sym_dict.get_mut(file_ns) {
      let v = n.to_owned();
      *n += 1;
      v
    } else {
      ns_sym_dict.insert(file_ns.into(), 2);
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
    info: Arc::new(CalcitSymbolInfo {
      at_ns: Arc::from(file_ns),
      at_def: calcit::GENERATED_DEF.into(),
    }),
    location: None,
  })
}
