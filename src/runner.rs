pub mod preprocess;
pub mod track;

use std::sync::Arc;
use std::vec;

use crate::builtins::{self, IMPORTED_PROCS};
use crate::calcit::{
  CORE_NS, Calcit, CalcitArgLabel, CalcitErr, CalcitErrKind, CalcitFn, CalcitFnArgs, CalcitImport, CalcitList, CalcitLocal, CalcitProc,
  CalcitScope, CalcitSyntax, MethodKind, NodeLocation,
};
use crate::call_stack::{CallStackList, StackKind, using_stack};
use crate::program;
use crate::util::string::has_ns_part;

pub fn evaluate_expr(expr: &Calcit, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  // println!("eval code: {}", expr.lisp_str());
  use Calcit::*;

  match expr {
    Nil
    | Bool(_)
    | Number(_)
    | Registered(_)
    | Tag(_)
    | Str(_)
    | Ref(..)
    | Tuple { .. }
    | Buffer(..)
    | CirruQuote(..)
    | Proc(_)
    | Macro { .. }
    | Fn { .. }
    | Syntax(_, _)
    | Method(..)
    | AnyRef(..) => Ok(expr.to_owned()),

    Thunk(thunk) => Ok(thunk.evaluated(scope, call_stack)?),
    Symbol { sym, info, location, .. } => {
      // println!("[Warn] slow path reading symbol: {}", sym);
      evaluate_symbol(sym, scope, &info.at_ns, &info.at_def, location, call_stack)
    }
    Local(CalcitLocal { idx, .. }) => evaluate_symbol_from_scope(*idx, scope),
    Import(CalcitImport { ns, def, coord, .. }) => evaluate_symbol_from_program(def, ns, *coord, call_stack),
    List(xs) => match xs.first() {
      None => Err(CalcitErr::use_msg_stack(
        CalcitErrKind::Arity,
        format!("cannot evaluate empty expr: {expr}"),
        call_stack,
      )),
      Some(x) => {
        // println!("eval expr: {}", expr.lisp_str());
        // println!("eval expr x: {}", x);

        if x.is_expr_evaluated() {
          call_expr(x, xs, scope, file_ns, call_stack, false)
        } else {
          let v = evaluate_expr(x, scope, file_ns, call_stack)?;
          call_expr(&v, xs, scope, file_ns, call_stack, false)
        }
      }
    },
    Recur(_) => unreachable!("recur not expected to be from symbol"),
    RawCode(_, code) => unreachable!("raw code `{}` cannot be called", code),
    Set(_) => Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Unexpected,
      "unexpected set for expr",
      call_stack,
    )),
    Map(_) => Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Unexpected,
      "unexpected map for expr",
      call_stack,
    )),
    Record { .. } => Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Unexpected,
      "unexpected record for expr",
      call_stack,
    )),
  }
}

pub fn call_expr(
  v: &Calcit,
  xs: &CalcitList,
  scope: &CalcitScope,
  file_ns: &str,
  call_stack: &CallStackList,
  spreading: bool,
) -> Result<Calcit, CalcitErr> {
  // println!("calling expr: {}", xs);
  let rest_nodes = xs.drop_left();
  match v {
    Calcit::Proc(p) => {
      let values = if spreading {
        evaluate_spreaded_args(rest_nodes, scope, file_ns, call_stack)?
      } else {
        evaluate_args(rest_nodes, scope, file_ns, call_stack)?
      };
      builtins::handle_proc(*p, &values, call_stack)
    }
    Calcit::Syntax(s, def_ns) => {
      if using_stack() {
        let next_stack = call_stack.extend(def_ns, s.as_ref(), StackKind::Syntax, &Calcit::from(xs), &rest_nodes.to_vec());
        builtins::handle_syntax(s, &rest_nodes, scope, file_ns, &next_stack).map_err(|e| {
          if e.stack.is_empty() {
            let mut e2 = e;
            call_stack.clone_into(&mut e2.stack);
            e2
          } else {
            e
          }
        })
      } else {
        builtins::handle_syntax(s, &rest_nodes, scope, file_ns, call_stack)
      }
    }
    Calcit::Method(name, kind) => {
      if *kind == MethodKind::Invoke {
        let values = if spreading {
          evaluate_spreaded_args(rest_nodes, scope, file_ns, call_stack)?
        } else {
          evaluate_args(rest_nodes, scope, file_ns, call_stack)?
        };
        if using_stack() {
          let next_stack = call_stack.extend(file_ns, name, StackKind::Method, &Calcit::Nil, &values);
          builtins::meta::invoke_method(name, &values, &next_stack)
        } else {
          builtins::meta::invoke_method(name, &values, call_stack)
        }
      } else if *kind == MethodKind::KeywordAccess {
        if rest_nodes.len() == 1 {
          let obj = evaluate_expr(&rest_nodes[0], scope, file_ns, call_stack)?;
          let tag = evaluate_expr(&Calcit::tag(name), scope, file_ns, call_stack)?;
          if let Calcit::Map(m) = obj {
            match m.get(&tag) {
              Some(value) => Ok(value.to_owned()),
              None => Ok(Calcit::Nil),
            }
          } else {
            Err(CalcitErr::use_msg_stack(
              CalcitErrKind::Type,
              format!("expected a hashmap, got: {obj}"),
              call_stack,
            ))
          }
        } else {
          Err(CalcitErr::use_msg_stack(
            CalcitErrKind::Arity,
            format!("keyword-accessor takes only 1 argument, {xs}"),
            call_stack,
          ))
        }
      } else {
        CalcitErr::err_str(CalcitErrKind::Unexpected, format!("unknown method for rust runtime: {kind}"))
      }
    }
    Calcit::Fn { info, .. } => {
      let values = if spreading {
        evaluate_spreaded_args(rest_nodes, scope, file_ns, call_stack)?
      } else {
        evaluate_args(rest_nodes, scope, file_ns, call_stack)?
      };
      if using_stack() {
        let next_stack = call_stack.extend(&info.def_ns, &info.name, StackKind::Fn, &Calcit::from(xs), &values);
        run_fn_owned(values, info, &next_stack)
      } else {
        run_fn_owned(values, info, call_stack)
      }
    }
    Calcit::Macro { info, .. } => {
      println!(
        "[Warn] macro should already be handled during preprocessing: {}",
        &Calcit::from(xs.to_owned()).lisp_str()
      );

      let next_stack = if using_stack() {
        call_stack.extend(&info.def_ns, &info.name, StackKind::Macro, &Calcit::from(xs), &rest_nodes.to_vec())
      } else {
        call_stack.to_owned()
      };

      // TODO moving to preprocess
      let mut current_values: Vec<Calcit> = rest_nodes.to_vec();
      // println!("eval macro: {} {}", x, expr.lisp_str()));
      // println!("macro... {} {}", x, CrListWrap(current_values.to_owned()));

      let mut body_scope = CalcitScope::default();

      Ok(loop {
        // need to handle recursion
        bind_marked_args(&mut body_scope, &info.args, &current_values, call_stack)?;
        let code = evaluate_lines(&info.body.to_vec(), &body_scope, &info.def_ns, &next_stack)?;
        match code {
          Calcit::Recur(ys) => {
            current_values = ys;
          }
          _ => {
            // println!("gen code: {} {}", x, &code.lisp_str()));
            break evaluate_expr(&code, scope, file_ns, &next_stack)?;
          }
        }
      })
    }
    Calcit::Tag(k) => {
      if rest_nodes.len() == 1 {
        let v = evaluate_expr(&rest_nodes[0], scope, file_ns, call_stack)?;

        if let Calcit::Map(m) = v {
          match m.get(&Calcit::Tag(k.to_owned())) {
            Some(value) => Ok(value.to_owned()),
            None => Ok(Calcit::Nil),
          }
        } else {
          Err(CalcitErr::use_msg_stack(
            CalcitErrKind::Type,
            format!("expected a hashmap, got: {v}"),
            call_stack,
          ))
        }
      } else {
        Err(CalcitErr::use_msg_stack(
          CalcitErrKind::Arity,
          format!("tag only takes 1 argument, got: {rest_nodes}"),
          call_stack,
        ))
      }
    }
    Calcit::Registered(alias) => {
      // call directly to reduce clone
      let ps = IMPORTED_PROCS.read().expect("read procs");
      match ps.get(alias) {
        Some(f) => {
          let values = if spreading {
            evaluate_spreaded_args(rest_nodes, scope, file_ns, call_stack)?
          } else {
            evaluate_args(rest_nodes, scope, file_ns, call_stack)?
          };
          // weird, but it's faster to pass `values` than passing `&values`
          // also println slows down code a bit. could't figure out, didn't read asm either
          f(values, call_stack)
        }
        None => Err(CalcitErr::use_msg_stack(
          CalcitErrKind::Var,
          format!("cannot evaluate symbol directly: {file_ns}/{alias}"),
          call_stack,
        )),
      }
    }
    a => Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!("cannot be used as operator: {a:?} in {xs}"),
      call_stack,
      a.get_location(),
    )),
  }
}

pub fn evaluate_symbol(
  sym: &str,
  scope: &CalcitScope,
  file_ns: &str,
  at_def: &str,
  location: &Option<Arc<Vec<u8>>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let v = match parse_ns_def(sym) {
    Some((ns_part, def_part)) => match program::lookup_ns_target_in_import(file_ns, &ns_part) {
      Some(target_ns) => match eval_symbol_from_program(&def_part, &target_ns, call_stack) {
        Ok(v) => Ok(v.expect("value")),
        Err(e) => Err(e),
      },
      None => Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Var,
        format!("unknown ns target: {ns_part}/{def_part}"),
        call_stack,
        Some(NodeLocation::new(
          Arc::from(file_ns),
          Arc::from(at_def),
          location.to_owned().unwrap_or_default(),
        )),
      )),
    },
    None => {
      if let Ok(v) = sym.parse::<CalcitSyntax>() {
        Ok(Calcit::Syntax(v, file_ns.into()))
      } else if let Some(v) = scope.get_by_name(sym) {
        // although scope is detected first, it would trigger warning during preprocess
        Ok(v.to_owned())
      } else if let Ok(p) = sym.parse::<CalcitProc>() {
        Ok(Calcit::Proc(p))
      } else if let Some(v) = eval_symbol_from_program(sym, CORE_NS, call_stack)? {
        Ok(v)
      } else if let Some(v) = eval_symbol_from_program(sym, file_ns, call_stack)? {
        Ok(v)
      } else if let Some(target_ns) = program::lookup_def_target_in_import(file_ns, sym) {
        eval_symbol_from_program(sym, &target_ns, call_stack).map(|v| v.expect("value"))
      } else {
        let vars = scope.get_names();
        Err(CalcitErr::use_msg_stack_location(
          CalcitErrKind::Var,
          format!("unknown symbol `{sym}` in {vars}"),
          call_stack,
          Some(NodeLocation::new(
            Arc::from(file_ns),
            Arc::from(at_def),
            location.to_owned().unwrap_or_default(),
          )),
        ))
      }
    }
  }?;
  match v {
    Calcit::Thunk(thunk) => thunk.evaluated(scope, call_stack),
    _ => Ok(v),
  }
}

pub fn evaluate_symbol_from_scope(idx: u16, scope: &CalcitScope) -> Result<Calcit, CalcitErr> {
  // although scope is detected first, it would trigger warning during preprocess
  Ok(
    scope
      .get(idx)
      .expect("expected symbol from scope, this is a quick path, should succeed")
      .to_owned(),
  )
}

/// a quick path of evaluating symbols, without checking scope and import
pub fn evaluate_symbol_from_program(
  sym: &str,
  file_ns: &str,
  coord: Option<(u16, u16)>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let v0 = match coord {
    Some((ns_idx, def_idx)) => program::load_by_index(ns_idx, file_ns, def_idx, sym),
    None => None,
  };
  // if v0.is_none() {
  //   println!("slow path reading symbol: {}/{}", file_ns, sym)
  // }
  let v = if let Some(v) = v0 {
    v
  } else if let Some(v) = eval_symbol_from_program(sym, CORE_NS, call_stack)? {
    v
  } else if file_ns == CORE_NS {
    if let Some(v) = eval_symbol_from_program(sym, CORE_NS, call_stack)? {
      v
    } else {
      unreachable!("expected symbol from path, this is a quick path, should succeed")
    }
  } else if let Some(v) = eval_symbol_from_program(sym, file_ns, call_stack)? {
    v
  } else {
    unreachable!("expected symbol from path, this is a quick path, should succeed")
  };
  match v {
    Calcit::Thunk(thunk) => thunk.evaluated(&CalcitScope::default(), call_stack),
    _ => Ok(v),
  }
}

pub fn parse_ns_def(s: &str) -> Option<(Arc<str>, Arc<str>)> {
  if !has_ns_part(s) {
    return None;
  }
  let pieces: Vec<&str> = s.split('/').collect();
  if pieces.len() == 2 {
    if !pieces[0].is_empty() && !pieces[1].is_empty() {
      Some((pieces[0].into(), pieces[1].into()))
    } else {
      None
    }
  } else {
    None
  }
}

/// without unfolding thunks
pub fn eval_symbol_from_program(sym: &str, ns: &str, call_stack: &CallStackList) -> Result<Option<Calcit>, CalcitErr> {
  if let Some(v) = program::lookup_evaled_def(ns, sym) {
    return Ok(Some(v));
  }
  if let Some(code) = program::lookup_def_code(ns, sym) {
    let v = evaluate_expr(&code, &CalcitScope::default(), ns, call_stack)?;
    program::write_evaled_def(ns, sym, v.to_owned()).map_err(|e| CalcitErr::use_msg_stack(CalcitErrKind::Unexpected, e, call_stack))?;
    return Ok(Some(v));
  }
  Ok(None)
}

pub fn run_fn(values: &[Calcit], info: &CalcitFn, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let mut body_scope = (*info.scope).to_owned();
  match &*info.args {
    CalcitFnArgs::Args(args) => {
      if args.len() != values.len() {
        unreachable!("args length mismatch")
      }
      for (idx, v) in args.iter().enumerate() {
        body_scope.insert_mut(*v, values[idx].to_owned());
      }
    }
    CalcitFnArgs::MarkedArgs(args) => bind_marked_args(&mut body_scope, args, values, call_stack)?,
  }

  let v = evaluate_lines(&info.body.to_vec(), &body_scope, &info.def_ns, call_stack)?;

  if let Calcit::Recur(xs) = v {
    let mut current_values = xs.to_vec();
    loop {
      match &*info.args {
        CalcitFnArgs::Args(args) => {
          if args.len() != current_values.len() {
            unreachable!("args length mismatch in recur")
          }
          for (idx, v) in args.iter().enumerate() {
            body_scope.insert_mut(*v, current_values[idx].to_owned());
          }
        }
        CalcitFnArgs::MarkedArgs(args) => bind_marked_args(&mut body_scope, args, &current_values, call_stack)?,
      }
      let v = evaluate_lines(&info.body.to_vec(), &body_scope, &info.def_ns, call_stack)?;
      match v {
        Calcit::Recur(xs) => current_values = xs.to_vec(),
        result => return Ok(result),
      }
    }
  }
  Ok(v)
}

/// quick path for `run_fn` which takes ownership of values
pub fn run_fn_owned(values: Vec<Calcit>, info: &CalcitFn, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let mut body_scope = (*info.scope).to_owned();
  match &*info.args {
    CalcitFnArgs::Args(args) => {
      if args.len() != values.len() {
        unreachable!("args length mismatch")
      }
      for (idx, v) in values.into_iter().enumerate() {
        body_scope.insert_mut(args[idx], v);
      }
    }
    CalcitFnArgs::MarkedArgs(args) => bind_marked_args(&mut body_scope, args, &values, call_stack)?,
  }

  let v = evaluate_lines(&info.body, &body_scope, &info.def_ns, call_stack)?;

  if let Calcit::Recur(xs) = v {
    let mut current_values = xs.to_vec();
    loop {
      match &*info.args {
        CalcitFnArgs::Args(args) => {
          if args.len() != current_values.len() {
            unreachable!("args length mismatch in recur")
          }
          for (idx, v) in current_values.into_iter().enumerate() {
            body_scope.insert_mut(args[idx], v);
          }
        }
        CalcitFnArgs::MarkedArgs(args) => bind_marked_args(&mut body_scope, args, &current_values, call_stack)?,
      }
      let v = evaluate_lines(&info.body, &body_scope, &info.def_ns, call_stack)?;
      match v {
        Calcit::Recur(xs) => current_values = xs.to_vec(),
        result => return Ok(result),
      }
    }
  }
  Ok(v)
}

/// syntax sugar for index value
#[derive(Debug, Default, PartialEq, PartialOrd)]
struct MutIndex(usize);

impl MutIndex {
  /// get value first, ant then increase value
  fn get_and_inc(&mut self) -> usize {
    let ret = self.0;
    self.0 += 1;
    ret
  }
}

/// create new scope by writing new args
/// notice that `&` is a mark for spreading, `?` for optional arguments
pub fn bind_marked_args(
  scope: &mut CalcitScope,
  args: &[CalcitArgLabel],
  values: &[Calcit],
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  // println!("bind args: {:?} {}", args, values);

  let mut spreading = false;
  let mut optional = false;

  let mut pop_args_idx = MutIndex::default();
  let mut pop_values_idx = MutIndex::default();

  while let Some(arg) = args.get(pop_args_idx.get_and_inc()) {
    if spreading {
      match arg {
        CalcitArgLabel::Idx(idx) => {
          let mut chunk: Vec<Calcit> = vec![];
          while let Some(v) = values.get(pop_values_idx.get_and_inc()) {
            chunk.push(v.to_owned());
          }
          scope.insert_mut(*idx, Calcit::from(CalcitList::Vector(chunk)));
          if pop_args_idx.0 < args.len() {
            return Err(CalcitErr::use_msg_stack(
              CalcitErrKind::Arity,
              format!("extra args `{args:?}` after spreading in `{args:?}`",),
              call_stack,
            ));
          }
        }
        _ => {
          return Err(CalcitErr::use_msg_stack(
            CalcitErrKind::Arity,
            format!("invalid control insode spreading mode: {args:?}"),
            call_stack,
          ));
        }
      }
    } else {
      match arg {
        CalcitArgLabel::RestMark => spreading = true,
        CalcitArgLabel::OptionalMark => optional = true,
        CalcitArgLabel::Idx(idx) => match values.get(pop_values_idx.get_and_inc()) {
          Some(v) => {
            scope.insert_mut(*idx, v.to_owned());
          }
          None => {
            if optional {
              scope.insert_mut(*idx, Calcit::Nil);
            } else {
              return Err(CalcitErr::use_msg_stack(
                CalcitErrKind::Arity,
                format!("too few values `{values:?}` passed to args `{args:?}`"),
                call_stack,
              ));
            }
          }
        },
      }
    }
  }

  if pop_values_idx.0 >= values.len() {
    Ok(())
  } else {
    Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Arity,
      format!("extra args `{args:?}` not handled while passing values `{values:?}` to args `{args:?}`",),
      call_stack,
    ))
  }
}

pub fn evaluate_lines(lines: &[Calcit], scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let mut ret: Calcit = Calcit::Nil;
  for line in lines {
    match evaluate_expr(line, scope, file_ns, call_stack) {
      Ok(v) => ret = v,
      Err(e) => return Err(e),
    }
  }
  Ok(ret)
}

/// quick path evaluate symbols before calling a function, not need to check `&` for spreading
pub fn evaluate_args(
  items: CalcitList,
  scope: &CalcitScope,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<Vec<Calcit>, CalcitErr> {
  let mut ret: Vec<Calcit> = Vec::with_capacity(items.len());
  for item in &items {
    // if let Calcit::Syntax(CalcitSyntax::ArgSpread, _) = item {
    //   unreachable!("unexpected spread in args: {items}, should be handled before calling this")
    // }

    if item.is_expr_evaluated() {
      ret.push(item.to_owned());
    } else {
      let v = evaluate_expr(item, scope, file_ns, call_stack)?;
      ret.push(v);
    }
  }
  // println!("Evaluated args: {}", ret);
  Ok(ret)
}

// evaluate symbols before calling a function
/// notice that `&` is used to spread a list
pub fn evaluate_spreaded_args(
  items: CalcitList,
  scope: &CalcitScope,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<Vec<Calcit>, CalcitErr> {
  let mut ret: Vec<Calcit> = Vec::with_capacity(items.len());
  let mut spreading = false;

  items.traverse_result(&mut |item| match item {
    Calcit::Syntax(CalcitSyntax::ArgSpread, _) => {
      spreading = true;
      Ok(())
    }
    _ => {
      if item.is_expr_evaluated() {
        if spreading {
          match item {
            Calcit::List(xs) => {
              xs.traverse(&mut |x| {
                ret.push(x.to_owned());
              });
              spreading = false;
              Ok(())
            }
            a => Err(CalcitErr::use_msg_stack(
              CalcitErrKind::Arity,
              format!("expected list for spreading, got: {a}"),
              call_stack,
            )),
          }
        } else {
          ret.push(item.to_owned());
          Ok(())
        }
      } else {
        let v = evaluate_expr(item, scope, file_ns, call_stack)?;

        if spreading {
          match v {
            Calcit::List(xs) => {
              xs.traverse(&mut |x| {
                ret.push(x.to_owned());
              });
              spreading = false;
              Ok(())
            }
            a => Err(CalcitErr::use_msg_stack(
              CalcitErrKind::Arity,
              format!("expected list for spreading, got: {a}"),
              call_stack,
            )),
          }
        } else {
          ret.push(v);
          Ok(())
        }
      }
    }
  })?;
  // println!("Evaluated args: {}", ret);
  Ok(ret)
}
