//! this file does not cover all syntax instances.
//! syntaxes related to data are maintained the corresponding files
//! Rust has limits on Closures, callbacks need to be handled specifically

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::vec;

use crate::builtins;
use crate::builtins::meta::{NS_SYMBOL_DICT, type_of};
use crate::calcit::{
  self, CalcitArgLabel, CalcitErrKind, CalcitFn, CalcitFnArgs, CalcitList, CalcitLocal, CalcitMacro, CalcitSymbolInfo, CalcitSyntax,
  CalcitTypeAnnotation, LocatedWarning,
};
use crate::calcit::{Calcit, CalcitErr, CalcitScope, gen_core_id};
use crate::call_stack::CallStackList;
use crate::runner::{self, call_expr, evaluate_expr};

pub fn defn(expr: &CalcitList, scope: &CalcitScope, file_ns: &str) -> Result<Calcit, CalcitErr> {
  match (expr.first(), expr.get(1)) {
    (Some(Calcit::Symbol { sym: s, .. }), Some(Calcit::List(xs))) => {
      let body_items = expr.skip(2)?.to_vec();
      let return_type = detect_return_type_hint(&body_items);
      let parsed_args = get_raw_args_fn(xs)?;
      let arg_types = parsed_args.empty_arg_types();
      Ok(Calcit::Fn {
        id: gen_core_id(),
        info: Arc::new(CalcitFn {
          name: s.to_owned(),
          def_ns: Arc::from(file_ns),
          scope: Arc::new(scope.to_owned()),
          args: Arc::new(parsed_args),
          body: body_items,
          return_type,
          arg_types,
        }),
      })
    }
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("defn expected a symbol and a list of arguments, but received: {a} , {b}"),
    ),
    _ => CalcitErr::err_str(
      CalcitErrKind::Arity,
      "defn expected a symbol and a list of arguments, but received insufficient arguments",
    ),
  }
}

pub fn defmacro(expr: &CalcitList, _scope: &CalcitScope, def_ns: &str) -> Result<Calcit, CalcitErr> {
  match (expr.first(), expr.get(1)) {
    (Some(Calcit::Symbol { sym: s, .. }), Some(Calcit::List(xs))) => Ok(Calcit::Macro {
      id: gen_core_id(),
      info: Arc::new(CalcitMacro {
        name: s.to_owned(),
        def_ns: Arc::from(def_ns),
        args: Arc::new(get_raw_args(xs)?),
        body: Arc::new(expr.skip(2)?.to_vec()),
      }),
    }),
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("defmacro expected a symbol and a list of arguments, but received: {a} {b}"),
    ),
    _ => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!(
        "defmacro expected a symbol and a list of arguments, but received: {}",
        Calcit::from(expr.to_owned())
      ),
    ),
  }
}

fn detect_return_type_hint(forms: &[Calcit]) -> Arc<CalcitTypeAnnotation> {
  for form in forms {
    if let Some(hint) = extract_return_type_from_hint(form) {
      return hint;
    }
  }
  Arc::new(CalcitTypeAnnotation::Dynamic)
}

fn extract_return_type_from_hint(form: &Calcit) -> Option<Arc<CalcitTypeAnnotation>> {
  let list = match form {
    Calcit::List(xs) => xs,
    _ => return None,
  };
  match list.first() {
    Some(Calcit::Syntax(CalcitSyntax::HintFn, _)) => {}
    _ => return None,
  }

  match list.get(1) {
    Some(Calcit::List(args)) => extract_return_type_from_args(args),
    _ => None,
  }
}

fn extract_return_type_from_args(args: &CalcitList) -> Option<Arc<CalcitTypeAnnotation>> {
  let items = args.to_vec();
  let mut idx = 0;
  while idx < items.len() {
    match &items[idx] {
      Calcit::Symbol { sym, .. } if &**sym == "return-type" => {
        if let Some(type_expr) = items.get(idx + 1) {
          return Some(CalcitTypeAnnotation::parse_type_annotation_form(type_expr));
        }
      }
      Calcit::List(inner) => {
        if let Some(found) = extract_return_type_from_args(inner) {
          return Some(found);
        }
      }
      _ => {}
    }
    idx += 1;
  }
  None
}

#[cfg(test)]
mod tests {
  use super::*;
  use cirru_edn::EdnTag;

  #[test]
  fn detects_return_type_from_hint() {
    let ns = "tests.fn";
    let ret_sym = make_symbol("return-type", ns, "demo");
    let type_expr = Calcit::Tag(EdnTag::from("number"));
    let hint_form = make_hint_form(ns, vec![ret_sym, type_expr.to_owned()]);

    let detected = detect_return_type_hint(&[hint_form]);
    assert!(matches!(detected.as_ref(), CalcitTypeAnnotation::Tag(_)), "should capture tag type");
  }

  #[test]
  fn ignores_flat_return_type_hint() {
    let ns = "tests.fn";
    let ret_sym = make_symbol("return-type", ns, "demo");
    let type_expr = Calcit::Tag(EdnTag::from("number"));
    let nodes = vec![Calcit::Syntax(CalcitSyntax::HintFn, Arc::from(ns)), ret_sym, type_expr];
    let flat_hint = Calcit::List(Arc::new(CalcitList::Vector(nodes)));

    assert!(
      matches!(*detect_return_type_hint(&[flat_hint]), CalcitTypeAnnotation::Dynamic),
      "flat form should be ignored"
    );
  }

  #[test]
  fn defn_captures_return_type_hint() {
    let ns = "tests.fn";
    let scope = CalcitScope::default();
    let fn_name = make_symbol("add1", ns, "main");
    let arg_local = make_local("x", ns, "main");
    let args_list = Calcit::List(Arc::new(CalcitList::Vector(vec![arg_local])));
    let hint_form = make_hint_form(
      ns,
      vec![make_symbol("return-type", ns, "main"), Calcit::Tag(EdnTag::from("number"))],
    );
    let body_expr = make_symbol("x", ns, "main");

    let expr = CalcitList::Vector(vec![fn_name, args_list, hint_form, body_expr]);

    let resolved = defn(&expr, &scope, ns).expect("defn should succeed");
    match resolved {
      Calcit::Fn { info, .. } => {
        assert!(matches!(info.return_type.as_ref(), CalcitTypeAnnotation::Tag(_)));
        assert_eq!(info.arg_types.len(), 1, "single parameter function should track one arg type slot");
        assert!(info.arg_types.iter().all(|slot| matches!(**slot, CalcitTypeAnnotation::Dynamic)));
      }
      other => panic!("expected function, got {other}"),
    }
  }

  fn make_symbol(name: &str, ns: &str, def: &str) -> Calcit {
    Calcit::Symbol {
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from(ns),
        at_def: Arc::from(def),
      }),
      location: None,
    }
  }

  fn make_local(name: &str, ns: &str, def: &str) -> Calcit {
    Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from(name)),
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from(ns),
        at_def: Arc::from(def),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Dynamic),
    })
  }

  fn make_hint_form(ns: &str, args: Vec<Calcit>) -> Calcit {
    let nodes = vec![
      Calcit::Syntax(CalcitSyntax::HintFn, Arc::from(ns)),
      Calcit::List(Arc::new(CalcitList::Vector(args))),
    ];
    Calcit::List(Arc::new(CalcitList::Vector(nodes)))
  }
}

pub fn get_raw_args(args: &CalcitList) -> Result<Vec<CalcitArgLabel>, String> {
  let mut xs: Vec<CalcitArgLabel> = vec![];
  args.traverse_result(&mut |item| match item {
    Calcit::Local(CalcitLocal { idx, .. }) => {
      xs.push(CalcitArgLabel::Idx(*idx));
      Ok(())
    }
    Calcit::Syntax(CalcitSyntax::ArgOptional, _) => {
      xs.push(CalcitArgLabel::OptionalMark);
      Ok(())
    }
    Calcit::Syntax(CalcitSyntax::ArgSpread, _) => {
      xs.push(CalcitArgLabel::RestMark);
      Ok(())
    }
    Calcit::Symbol { sym, .. } => {
      let idx = CalcitLocal::track_sym(sym);
      xs.push(CalcitArgLabel::Idx(idx));
      Ok(())
    }
    _ => Err(format!("get-raw-args unexpected argument: {item}")),
  })?;
  Ok(xs)
}

pub fn get_raw_args_fn(args: &CalcitList) -> Result<CalcitFnArgs, String> {
  let mut xs: Vec<CalcitArgLabel> = vec![];
  let mut has_mark = false;
  args.traverse_result(&mut |item| {
    match item {
      Calcit::Local(CalcitLocal { idx, .. }) => {
        xs.push(CalcitArgLabel::Idx(*idx));
        Ok(())
      }
      Calcit::Syntax(CalcitSyntax::ArgSpread, _) => {
        xs.push(CalcitArgLabel::RestMark);
        has_mark = true;
        Ok(())
      }
      Calcit::Syntax(CalcitSyntax::ArgOptional, _) => {
        xs.push(CalcitArgLabel::OptionalMark);
        has_mark = true;
        Ok(())
      }
      Calcit::Symbol { sym, .. } => {
        let idx = CalcitLocal::track_sym(sym);
        // during macro processing, we still git symbol
        xs.push(CalcitArgLabel::Idx(idx));
        Ok(())
      }
      _ => Err(format!("get-raw-args-fn unexpected argument: {item:?}")),
    }
  })?;
  if has_mark {
    Ok(CalcitFnArgs::MarkedArgs(xs))
  } else {
    let mut ys: Vec<u16> = Vec::with_capacity(xs.len());
    for x in &xs {
      match x {
        CalcitArgLabel::Idx(idx) => {
          ys.push(*idx);
        }
        _ => return Err(format!("get-raw-args-fn unexpected argument: {x}")),
      }
    }
    Ok(CalcitFnArgs::Args(ys))
  }
}

pub fn quote(expr: &CalcitList, _scope: &CalcitScope, _file_ns: &str) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    Ok(expr[0].to_owned())
  } else {
    CalcitErr::err_nodes(CalcitErrKind::Arity, "quote expected 1 argument, but received:", &expr.to_vec())
  }
}

pub fn syntax_if(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let l = expr.len();
  if l > 3 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "if expected at most 3 arguments, but received:",
      &expr.to_vec(),
    );
  }
  if l < 2 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "if expected at least 2 arguments, but received:",
      &expr.to_vec(),
    );
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
    CalcitErr::err_nodes(CalcitErrKind::Arity, "eval expected 1 argument, but received:", &expr.to_vec())
  }
}

pub fn syntax_let(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  match expr.first() {
    // Some(Calcit::Nil) => runner::evaluate_lines(&expr.drop_left(), scope, file_ns, call_stack),
    Some(Calcit::List(xs)) if xs.is_empty() => runner::evaluate_lines(&expr.drop_left().to_vec(), scope, file_ns, call_stack),
    Some(Calcit::List(xs)) if xs.len() == 2 => {
      let mut body_scope = scope.to_owned();
      match (&xs[0], &xs[1]) {
        (Calcit::Local(CalcitLocal { idx, .. }), ys) => {
          let value = runner::evaluate_expr(ys, scope, file_ns, call_stack)?;
          body_scope.insert_mut(*idx, value);
        }
        (Calcit::Symbol { sym: s, .. }, ys) => {
          eprintln!("[Warn] slow path of {s}, prefer local");
          let value = runner::evaluate_expr(ys, scope, file_ns, call_stack)?;
          let idx = CalcitLocal::track_sym(s);
          body_scope.insert_mut(idx, value);
        }
        (a, _) => return CalcitErr::err_str(CalcitErrKind::Type, format!("let invalid binding name: {a}")),
      }
      runner::evaluate_lines(&expr.drop_left().to_vec(), &body_scope, file_ns, call_stack)
    }
    Some(Calcit::List(xs)) => CalcitErr::err_nodes(CalcitErrKind::Arity, "let invalid length, but received:", &xs.to_vec()),
    Some(_) => CalcitErr::err_str(CalcitErrKind::Type, format!("let invalid node, but received: {}", expr.to_owned())),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "let expected a pair or nil, but received none"),
  }
}

// code replaced from `~` and `~@` returns different results
#[derive(Clone, PartialEq, Debug)]
enum SpanResult {
  Single(Calcit),
  Range(Arc<CalcitList>),
}

pub fn quasiquote(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  match expr.first() {
    None => CalcitErr::err_str(CalcitErrKind::Arity, "quasiquote expected a node, but received none"),
    Some(code) => {
      match replace_code(code, scope, file_ns, call_stack)? {
        SpanResult::Single(v) => {
          // println!("replace result: {:?}", v);
          Ok(v)
        }
        SpanResult::Range(xs) => CalcitErr::err_nodes(
          CalcitErrKind::Arity,
          "quasiquote expected single result, but received:",
          &xs.to_vec(),
        ),
      }
    }
  }
}

fn replace_code(c: &Calcit, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<SpanResult, CalcitErr> {
  if !has_unquote(c) {
    return Ok(SpanResult::Single(c.to_owned()));
  }
  match c {
    Calcit::List(ys) => match (ys.first(), ys.get(1)) {
      (Some(Calcit::Syntax(CalcitSyntax::MacroInterpolate, _)), Some(expr)) => {
        let value = runner::evaluate_expr(expr, scope, file_ns, call_stack)?;
        Ok(SpanResult::Single(value))
      }
      (Some(Calcit::Syntax(CalcitSyntax::MacroInterpolateSpread, _)), Some(expr)) => {
        let ret = runner::evaluate_expr(expr, scope, file_ns, call_stack)?;
        match ret {
          Calcit::List(zs) => Ok(SpanResult::Range(zs.to_owned())),
          _ => Err(CalcitErr::use_str(
            CalcitErrKind::Type,
            format!("unquote-slice unknown result, but received: {ret}"),
          )),
        }
      }
      (_, _) => {
        let mut ret: Vec<Calcit> = vec![];
        ys.traverse_result::<CalcitErr>(&mut |y| match replace_code(y, scope, file_ns, call_stack)? {
          SpanResult::Single(z) => {
            ret.push(z);
            Ok(())
          }
          SpanResult::Range(pieces) => {
            pieces.traverse(&mut |z| {
              ret.push(z.to_owned());
            });
            Ok(())
          }
        })?;
        Ok(SpanResult::Single(Calcit::from(CalcitList::Vector(ret))))
      }
    },
    _ => Ok(SpanResult::Single(c.to_owned())),
  }
}

pub fn has_unquote(xs: &Calcit) -> bool {
  match xs {
    Calcit::List(ys) => {
      for y in &**ys {
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
              let v = runner::evaluate_lines(&info.body.to_vec(), &body_scope, &info.def_ns, call_stack)?;
              match v {
                Calcit::Recur(rest_code) => {
                  (*rest_code).clone_into(&mut rest_nodes);
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
    CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "macroexpand expected 1 argument, but received:",
      &expr.to_vec(),
    )
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
            runner::evaluate_lines(&info.body.to_vec(), &body_scope, &info.def_ns, call_stack)
          }
          _ => Ok(quoted_code),
        }
      }
      a => Ok(a.to_owned()),
    }
  } else {
    CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "macroexpand-1 expected 1 argument, but received:",
      &expr.to_vec(),
    )
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
              let v = runner::evaluate_lines(&info.body.to_vec(), &body_scope, &info.def_ns, call_stack)?;
              match v {
                Calcit::Recur(rest_code) => {
                  rest_nodes = (*rest_code).to_vec();
                }
                _ => {
                  let mut scope_types = HashMap::new();
                  let resolved =
                    runner::preprocess::preprocess_expr(&v, &HashSet::new(), &mut scope_types, file_ns, check_warnings, call_stack)?;
                  let warnings = check_warnings.borrow();
                  LocatedWarning::print_list(&warnings);

                  return Ok(resolved);
                }
              }
            }
          }
          _ => {
            let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
            let mut scope_types = HashMap::new();
            let resolved = runner::preprocess::preprocess_expr(
              &quoted_code,
              &HashSet::new(),
              &mut scope_types,
              file_ns,
              check_warnings,
              call_stack,
            )?;
            LocatedWarning::print_list(&check_warnings.borrow());
            Ok(resolved)
          }
        }
      }
      a => Ok(a.to_owned()),
    }
  } else {
    CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "macroexpand-all expected 1 argument, but received:",
      &expr.to_vec(),
    )
  }
}

/// inserted automatically when `&` syntax is recognized in calling
pub fn call_spread(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if expr.len() < 3 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "call-spread expected at least 3 arguments, but received:",
      &expr.to_vec(),
    );
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
          a => {
            let msg = format!(
              "try requires a function handler, but received: {}",
              type_of(&[a.to_owned()])?.lisp_str()
            );
            CalcitErr::err_str(CalcitErrKind::Type, msg)
          }
        }
      }
    }
  } else {
    CalcitErr::err_nodes(CalcitErrKind::Arity, "try expected 2 arguments, but received:", &expr.to_vec())
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
      a => {
        let msg = format!(
          "gensym requires a string/symbol/tag, but received: {}",
          type_of(&[a.to_owned()])?.lisp_str()
        );
        return CalcitErr::err_str(CalcitErrKind::Type, msg);
      }
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
