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
  self, CalcitArgLabel, CalcitErrKind, CalcitFn, CalcitFnArgs, CalcitFnDefRef, CalcitFnUsageMeta, CalcitList, CalcitLocal, CalcitMacro,
  CalcitSymbolInfo, CalcitSyntax, CalcitTypeAnnotation, LocatedWarning,
};
use crate::calcit::{Calcit, CalcitErr, CalcitScope, gen_core_id};
use crate::call_stack::CallStackList;
use crate::runner::{self, call_expr, evaluate_expr};

pub fn defn(expr: &CalcitList, scope: &CalcitScope, file_ns: &str) -> Result<Calcit, CalcitErr> {
  match (expr.first(), expr.get(1)) {
    (Some(Calcit::Symbol { sym: s, .. }), Some(Calcit::List(xs))) => {
      let body_items = expr.skip(2)?.to_vec();
      let return_type = detect_return_type_hint(&body_items);
      let generics = detect_fn_generics(&body_items);
      let parsed_args = get_raw_args_fn(xs)?;
      let param_symbols = match collect_param_symbols(xs) {
        Ok(params) => params,
        Err(err) => {
          return CalcitErr::err_str(CalcitErrKind::Type, format!("defn args parse error: {err}"));
        }
      };
      // Highest priority: extract arg types from an injected schema hint-fn form.
      // The schema is injected during preprocessing via `program::lookup_def_schema`.
      let mut arg_types = body_items
        .iter()
        .find_map(|f| CalcitTypeAnnotation::extract_arg_types_from_hint_form(f, &param_symbols))
        .unwrap_or_else(|| CalcitTypeAnnotation::collect_arg_type_hints_from_body(&body_items, &param_symbols));
      // Fallback: if all arg_types are Dynamic (assert-type was preprocessed away),
      // extract types from Local nodes in the preprocessed args list
      if file_ns != calcit::CORE_NS && arg_types.iter().all(|t| matches!(t.as_ref(), CalcitTypeAnnotation::Dynamic)) {
        let from_locals = extract_arg_types_from_locals(xs, &param_symbols);
        if from_locals.iter().any(|t| !matches!(t.as_ref(), CalcitTypeAnnotation::Dynamic)) {
          arg_types = from_locals;
        }
      }
      if file_ns != calcit::CORE_NS && arg_types.iter().all(|t| matches!(t.as_ref(), CalcitTypeAnnotation::Dynamic)) {
        let from_body = extract_arg_types_from_processed_body(&body_items, &param_symbols);
        if from_body.iter().any(|t| !matches!(t.as_ref(), CalcitTypeAnnotation::Dynamic)) {
          arg_types = from_body;
        }
      }
      let is_macro_gen = s.as_ref().contains('%');
      Ok(Calcit::Fn {
        id: gen_core_id(),
        info: Arc::new(CalcitFn {
          name: s.to_owned(),
          def_ns: Arc::from(file_ns),
          def_ref: Some(CalcitFnDefRef {
            def_ns: Arc::from(file_ns),
            def_name: s.to_owned(),
            coord: None,
            is_defn: true,
            is_macro_gen,
          }),
          usage: CalcitFnUsageMeta::default(),
          scope: Arc::new(scope.to_owned()),
          args: Arc::new(parsed_args),
          body: body_items,
          generics,
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
    if let Some(hint) = CalcitTypeAnnotation::extract_return_type_from_hint_form(form) {
      return hint;
    }
  }
  crate::calcit::DYNAMIC_TYPE.clone()
}

fn detect_fn_generics(forms: &[Calcit]) -> Arc<Vec<Arc<str>>> {
  for form in forms {
    if let Some(vars) = CalcitTypeAnnotation::extract_generics_from_hint_form(form) {
      return Arc::new(vars);
    }
  }
  Arc::new(vec![])
}

/// Extract arg types from preprocessed Local nodes in the args list.
/// After preprocessing, `preprocess_defn` may update arg Local nodes with
/// resolved type_info from assert-type. This reads those types directly.
fn extract_arg_types_from_locals(args: &CalcitList, params: &[Arc<str>]) -> Vec<Arc<CalcitTypeAnnotation>> {
  let mut result = vec![crate::calcit::DYNAMIC_TYPE.clone(); params.len()];
  let mut param_idx = 0;
  for item in args.iter() {
    if let Calcit::Local(local) = item {
      if param_idx < params.len() && local.sym == params[param_idx] {
        result[param_idx] = local.type_info.clone();
        param_idx += 1;
      }
    } else if matches!(
      item,
      Calcit::Syntax(CalcitSyntax::ArgSpread, _) | Calcit::Syntax(CalcitSyntax::ArgOptional, _)
    ) {
      // Skip marker syntax nodes
      continue;
    }
  }
  result
}

/// Extract arg types from top-level preprocessed body forms.
/// `assert-type` on a local becomes a typed `Calcit::Local` form after preprocessing.
/// This intentionally scans only top-level forms to avoid deep recursive walks.
fn extract_arg_types_from_processed_body(forms: &[Calcit], params: &[Arc<str>]) -> Vec<Arc<CalcitTypeAnnotation>> {
  let mut result = vec![crate::calcit::DYNAMIC_TYPE.clone(); params.len()];
  for form in forms {
    if let Calcit::Local(local) = form {
      if matches!(local.type_info.as_ref(), CalcitTypeAnnotation::Dynamic) {
        continue;
      }
      if let Some(idx) = params.iter().position(|sym| sym == &local.sym) {
        result[idx] = local.type_info.clone();
      }
    }
  }
  result
}

fn collect_param_symbols(args: &CalcitList) -> Result<Vec<Arc<str>>, String> {
  let mut params: Vec<Arc<str>> = vec![];
  args.traverse_result(&mut |item| match item {
    Calcit::Local(CalcitLocal { sym, .. }) => {
      params.push(sym.to_owned());
      Ok(())
    }
    Calcit::Symbol { sym, .. } => {
      params.push(sym.to_owned());
      Ok(())
    }
    Calcit::Syntax(CalcitSyntax::ArgSpread, _) | Calcit::Syntax(CalcitSyntax::ArgOptional, _) => Ok(()),
    _ => Err(format!("collect-param-symbols unexpected argument: {item:?}")),
  })?;
  Ok(params)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitRecord, CalcitStruct, CalcitTrait};
  use crate::call_stack::CallStackList;
  use cirru_edn::EdnTag;

  #[test]
  fn detects_return_type_from_hint() {
    let ns = "tests.fn";
    let type_expr = Calcit::Tag(EdnTag::from("number"));
    let hint_form = make_hint_form(
      ns,
      vec![
        make_symbol("{}", ns, "demo"),
        Calcit::List(Arc::new(CalcitList::Vector(vec![
          Calcit::Tag(EdnTag::from("return")),
          type_expr.to_owned(),
        ]))),
      ],
    );

    let detected = detect_return_type_hint(&[hint_form]);
    assert!(
      matches!(detected.as_ref(), CalcitTypeAnnotation::Number),
      "should capture number type"
    );
  }

  #[test]
  fn ignores_flat_return_type_hint() {
    let ns = "tests.fn";
    let ret_sym = make_symbol("return", ns, "demo");
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
      vec![
        make_symbol("{}", ns, "main"),
        Calcit::List(Arc::new(CalcitList::Vector(vec![
          Calcit::Tag(EdnTag::from("return")),
          Calcit::Tag(EdnTag::from("number")),
        ]))),
      ],
    );
    let body_expr = make_symbol("x", ns, "main");

    let expr = CalcitList::Vector(vec![fn_name, args_list, hint_form, body_expr]);

    let resolved = defn(&expr, &scope, ns).expect("defn should succeed");
    match resolved {
      Calcit::Fn { info, .. } => {
        assert!(matches!(info.return_type.as_ref(), CalcitTypeAnnotation::Number));
        assert_eq!(info.arg_types.len(), 1, "single parameter function should track one arg type slot");
        assert!(info.arg_types.iter().all(|slot| matches!(**slot, CalcitTypeAnnotation::Dynamic)));
      }
      other => panic!("expected function, got {other}"),
    }
  }

  #[test]
  fn assert_type_runtime_checks_pass() {
    let scope = CalcitScope::default();
    let expr = CalcitList::Vector(vec![Calcit::Number(1.0), Calcit::Tag(EdnTag::from("number"))]);
    let result = assert_type(&expr, &scope, "tests.assert", &CallStackList::default()).expect("assert-type should pass");
    assert!(matches!(result, Calcit::Number(1.0)));
  }

  #[test]
  fn assert_type_runtime_checks_fail() {
    let scope = CalcitScope::default();
    let expr = CalcitList::Vector(vec![Calcit::Str(Arc::from("oops")), Calcit::Tag(EdnTag::from("number"))]);
    let err = assert_type(&expr, &scope, "tests.assert", &CallStackList::default()).expect_err("assert-type should fail");
    assert!(format!("{err}").contains("assert-type failed"));
  }

  #[test]
  fn assert_traits_runtime_checks_pass() {
    let mut scope = CalcitScope::default();
    let sym = Arc::from("x");
    let idx = CalcitLocal::track_sym(&sym);
    scope.insert_mut(
      idx,
      Calcit::Record(CalcitRecord {
        struct_ref: Arc::new(CalcitStruct::from_fields(EdnTag::from("Person"), vec![])),
        values: Arc::new(vec![]),
      }),
    );

    let empty_trait = Calcit::Trait(CalcitTrait::new(EdnTag::from("Noop"), vec![], vec![]));
    let expr = CalcitList::Vector(vec![
      Calcit::Local(CalcitLocal {
        idx,
        sym: Arc::from("x"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.assert"),
          at_def: Arc::from("main"),
        }),
        location: None,
        type_info: crate::calcit::DYNAMIC_TYPE.clone(),
      }),
      empty_trait,
    ]);
    let result = assert_traits(&expr, &scope, "tests.assert", &CallStackList::default()).expect("assert-traits should pass");
    assert!(matches!(result, Calcit::Record(_)));
  }

  #[test]
  fn assert_traits_runtime_checks_fail_on_non_trait() {
    let scope = CalcitScope::default();
    let expr = CalcitList::Vector(vec![Calcit::Number(1.0), Calcit::Tag(EdnTag::from("not-trait"))]);
    let err = assert_traits(&expr, &scope, "tests.assert", &CallStackList::default()).expect_err("assert-traits should fail");
    assert!(format!("{err}").contains("expected a trait definition"));
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
      type_info: crate::calcit::DYNAMIC_TYPE.clone(),
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
    let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
    let mut scope_types = HashMap::new();
    let resolved = runner::preprocess::preprocess_expr(&v, &HashSet::new(), &mut scope_types, file_ns, check_warnings, call_stack)?;
    LocatedWarning::print_list(&check_warnings.borrow());
    runner::evaluate_expr(&resolved, scope, file_ns, call_stack)
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

pub fn assert_type(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if expr.len() != 2 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "assert-type expected 2 arguments, but received:",
      &expr.to_vec(),
    );
  }

  let value = runner::evaluate_expr(&expr[0], scope, file_ns, call_stack)?;
  let expected = CalcitTypeAnnotation::parse_type_annotation_form(&expr[1]);

  if !calcit::value_matches_type_annotation(&value, expected.as_ref()) {
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!(
        "assert-type failed: expected `{}`, got `:{}` for value {value}",
        expected.to_brief_string(),
        calcit::brief_type_of_value(&value)
      ),
      call_stack,
      expr.first().and_then(|node| node.get_location()),
    ));
  }

  Ok(value)
}

pub fn assert_traits(expr: &CalcitList, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if expr.len() < 2 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "assert-traits expected at least 2 arguments, but received:",
      &expr.to_vec(),
    );
  }

  let mut value = runner::evaluate_expr(&expr[0], scope, file_ns, call_stack)?;

  for trait_form in expr.iter().skip(1) {
    let trait_value = runner::evaluate_expr(trait_form, scope, file_ns, call_stack)?;
    value = builtins::meta::assert_traits(&[value, trait_value], call_stack)?;
  }

  Ok(value)
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
  let (result, _touched) = replace_code_single_pass(c, scope, file_ns, call_stack)?;
  Ok(result)
}

fn replace_code_single_pass(
  c: &Calcit,
  scope: &CalcitScope,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<(SpanResult, bool), CalcitErr> {
  match c {
    Calcit::List(ys) => match (ys.first(), ys.get(1)) {
      (Some(Calcit::Syntax(CalcitSyntax::MacroInterpolate, _)), Some(expr)) => {
        let value = runner::evaluate_expr(expr, scope, file_ns, call_stack)?;
        Ok((SpanResult::Single(value), true))
      }
      (Some(Calcit::Syntax(CalcitSyntax::MacroInterpolateSpread, _)), Some(expr)) => {
        let ret = runner::evaluate_expr(expr, scope, file_ns, call_stack)?;
        match ret {
          Calcit::List(zs) => Ok((SpanResult::Range(zs.to_owned()), true)),
          _ => Err(CalcitErr::use_str(
            CalcitErrKind::Type,
            format!("unquote-slice unknown result, but received: {ret}"),
          )),
        }
      }
      (_, _) => {
        let mut ret: Vec<Calcit> = vec![];
        let mut touched = false;
        ys.traverse_result::<CalcitErr>(&mut |y| {
          let (piece, changed) = replace_code_single_pass(y, scope, file_ns, call_stack)?;
          touched = touched || changed;
          match piece {
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
          }
        })?;
        if touched {
          Ok((SpanResult::Single(Calcit::from(CalcitList::Vector(ret))), true))
        } else {
          Ok((SpanResult::Single(c.to_owned()), false))
        }
      }
    },
    _ => Ok((SpanResult::Single(c.to_owned()), false)),
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

fn detect_macro_head_name(code: &Calcit, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Option<Arc<str>> {
  let Calcit::List(xs) = code else {
    return None;
  };
  if xs.is_empty() {
    return None;
  }
  let head_value = runner::evaluate_expr(&xs[0], scope, file_ns, call_stack).ok()?;
  match head_value {
    Calcit::Macro { info, .. } => Some(info.name.to_owned()),
    _ => None,
  }
}

fn print_macroexpand_chain(label: &str, chain: &[Arc<str>]) {
  if chain.len() < 2 {
    return;
  }
  let chain_text = chain.iter().map(|s| s.as_ref()).collect::<Vec<_>>().join(" -> ");
  eprintln!("[{label}] expansion chain: {chain_text}");
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
            let mut chain: Vec<Arc<str>> = vec![info.name.to_owned()];
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
                _ => {
                  if let Some(next_macro) = detect_macro_head_name(&v, scope, file_ns, call_stack) {
                    chain.push(next_macro);
                  }
                  print_macroexpand_chain("macroexpand", &chain);
                  return Ok(v);
                }
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
            let mut chain: Vec<Arc<str>> = vec![info.name.to_owned()];
            let mut body_scope = scope.to_owned();
            runner::bind_marked_args(&mut body_scope, &info.args, &xs.drop_left().to_vec(), call_stack)?;
            let expanded = runner::evaluate_lines(&info.body.to_vec(), &body_scope, &info.def_ns, call_stack)?;
            if let Some(next_macro) = detect_macro_head_name(&expanded, scope, file_ns, call_stack) {
              chain.push(next_macro);
            }
            print_macroexpand_chain("macroexpand-1", &chain);
            Ok(expanded)
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
            let mut chain: Vec<Arc<str>> = vec![info.name.to_owned()];
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
                  if let Some(next_macro) = detect_macro_head_name(&v, scope, file_ns, call_stack) {
                    chain.push(next_macro);
                  }
                  print_macroexpand_chain("macroexpand-all", &chain);
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
