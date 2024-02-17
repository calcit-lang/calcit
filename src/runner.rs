pub mod preprocess;
pub mod track;

use im_ternary_tree::TernaryTreeList;
use std::sync::Arc;

use crate::builtins::{self, IMPORTED_PROCS};
use crate::calcit::{
  Calcit, CalcitArgLabel, CalcitErr, CalcitFn, CalcitImport, CalcitList, CalcitProc, CalcitScope, CalcitSyntax, MethodKind,
  NodeLocation, CORE_NS,
};
use crate::call_stack::{using_stack, CallStackList, StackKind};
use crate::program;
use crate::util::string::has_ns_part;

pub fn evaluate_expr(expr: &Calcit, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  // println!("eval code: {}", expr.lisp_str());

  match expr {
    Calcit::Nil
    | Calcit::Bool(_)
    | Calcit::Number(_)
    | Calcit::Registered(_)
    | Calcit::Tag(_)
    | Calcit::Str(_)
    | Calcit::Ref(..)
    | Calcit::Tuple { .. }
    | Calcit::Buffer(..)
    | Calcit::CirruQuote(..)
    | Calcit::Proc(_)
    | Calcit::Macro { .. }
    | Calcit::Fn { .. }
    | Calcit::Syntax(_, _)
    | Calcit::Method(..) => Ok(expr.to_owned()),

    Calcit::Symbol { sym, .. } if &**sym == "&" => Ok(expr.to_owned()),

    Calcit::Thunk(thunk) => Ok(thunk.evaluated(scope, call_stack)?),
    Calcit::Symbol { sym, info, location, .. } => {
      // println!("[Warn] slow path reading symbol: {}", sym);
      evaluate_symbol(sym, scope, &info.at_ns, &info.at_def, location, call_stack)
    }
    Calcit::Local { sym, .. } => evaluate_symbol_from_scope(sym, scope),
    Calcit::Import(CalcitImport { ns, def, coord, .. }) => {
      // TODO might have quick path
      evaluate_symbol_from_program(def, ns, *coord, call_stack)
    }
    Calcit::List(xs) => match xs.get(0) {
      None => Err(CalcitErr::use_msg_stack(format!("cannot evaluate empty expr: {expr}"), call_stack)),
      Some(x) => {
        // println!("eval expr: {}", expr.lisp_str());
        // println!("eval expr x: {}", x);

        if x.is_expr_evaluated() {
          call_expr(x, xs, scope, file_ns, call_stack)
        } else {
          let v = evaluate_expr(x, scope, file_ns, call_stack)?;
          call_expr(&v, xs, scope, file_ns, call_stack)
        }
      }
    },
    Calcit::Recur(_) => unreachable!("recur not expected to be from symbol"),
    Calcit::RawCode(_, code) => unreachable!("raw code `{}` cannot be called", code),
    Calcit::Set(_) => Err(CalcitErr::use_msg_stack("unexpected set for expr", call_stack)),
    Calcit::Map(_) => Err(CalcitErr::use_msg_stack("unexpected map for expr", call_stack)),
    Calcit::Record { .. } => Err(CalcitErr::use_msg_stack("unexpected record for expr", call_stack)),
  }
}

pub fn call_expr(
  v: &Calcit,
  xs: &CalcitList,
  scope: &CalcitScope,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let rest_nodes = xs.drop_left();
  match v {
    Calcit::Proc(p) => {
      let values = evaluate_args(rest_nodes, scope, file_ns, call_stack)?;
      builtins::handle_proc(*p, values, call_stack)
    }
    Calcit::Syntax(s, def_ns) => {
      let next_stack = if using_stack() {
        call_stack.extend(def_ns, s.as_ref(), StackKind::Syntax, &Calcit::from(xs.to_owned()), &rest_nodes.0)
      } else {
        call_stack.to_owned()
      };

      builtins::handle_syntax(s, &rest_nodes, scope, file_ns, &next_stack).map_err(|e| {
        if e.stack.is_empty() {
          let mut e2 = e;
          e2.stack = call_stack.to_owned();
          e2
        } else {
          e
        }
      })
    }
    Calcit::Method(name, kind) => {
      let values = evaluate_args(rest_nodes, scope, file_ns, call_stack)?;
      let next_stack = if using_stack() {
        call_stack.extend(file_ns, name, StackKind::Method, &Calcit::Nil, &values)
      } else {
        call_stack.to_owned()
      };

      if *kind == MethodKind::Invoke {
        builtins::meta::invoke_method(name, &values, &next_stack)
      } else {
        CalcitErr::err_str(format!("unknown method for rust runtime: {kind}"))
      }
    }
    Calcit::Fn { info, .. } => {
      let values = evaluate_args(rest_nodes, scope, file_ns, call_stack)?;
      let next_stack = if using_stack() {
        call_stack.extend(&info.def_ns, &info.name, StackKind::Fn, &Calcit::from(xs.to_owned()), &values)
      } else {
        call_stack.to_owned()
      };

      run_fn(values, info, &next_stack)
    }
    Calcit::Macro { info, .. } => {
      println!(
        "[Warn] macro should already be handled during preprocessing: {}",
        &Calcit::from(xs.to_owned()).lisp_str()
      );

      // TODO moving to preprocess
      let mut current_values: TernaryTreeList<Calcit> = rest_nodes.clone().into();
      // println!("eval macro: {} {}", x, expr.lisp_str()));
      // println!("macro... {} {}", x, CrListWrap(current_values.to_owned()));

      let next_stack = if using_stack() {
        call_stack.extend(
          &info.def_ns,
          &info.name,
          StackKind::Macro,
          &Calcit::from(xs.to_owned()),
          &rest_nodes.0,
        )
      } else {
        call_stack.to_owned()
      };

      let mut body_scope = CalcitScope::default();

      Ok(loop {
        // need to handle recursion
        bind_args(&mut body_scope, &info.args, &current_values, call_stack)?;
        let code = evaluate_lines(&info.body, &body_scope, &info.def_ns, &next_stack)?;
        match code {
          Calcit::Recur(ys) => {
            current_values = (*ys).to_owned();
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
          Err(CalcitErr::use_msg_stack(format!("expected a hashmap, got: {v}"), call_stack))
        }
      } else {
        Err(CalcitErr::use_msg_stack(
          format!("tag only takes 1 argument, got: {}", rest_nodes),
          call_stack,
        ))
      }
    }
    Calcit::Registered(alias) => {
      let ps = IMPORTED_PROCS.read().expect("read procs");
      match ps.get(alias) {
        Some(f) => {
          let values = evaluate_args(rest_nodes, scope, file_ns, call_stack)?;
          // weird, but it's faster to pass `values` than passing `&values`
          // also println slows down code a bit. could't figure out, didn't read asm either
          f(values, call_stack)
        }
        None => Err(CalcitErr::use_msg_stack(
          format!("cannot evaluate symbol directly: {file_ns}/{alias}"),
          call_stack,
        )),
      }
    }
    a => Err(CalcitErr::use_msg_stack_location(
      format!("cannot be used as operator: {a:?} in {}", xs),
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
      } else if let Some(v) = scope.get(sym) {
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
          format!("unknown symbol `{sym}` in {}", vars),
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

pub fn evaluate_symbol_from_scope(sym: &str, scope: &CalcitScope) -> Result<Calcit, CalcitErr> {
  // although scope is detected first, it would trigger warning during preprocess
  Ok(
    scope
      .get(sym)
      .expect("expected symbol from scope, this is a quick path, should succeed")
      .to_owned(),
  )
}

/// a quick path of evaluating symbols, without checking scope and import
pub fn evaluate_symbol_from_program(
  sym: &str,
  file_ns: &str,
  coord: Option<(usize, usize)>,
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
      Some((pieces[0].to_owned().into(), pieces[1].to_owned().into()))
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
    program::write_evaled_def(ns, sym, v.to_owned()).map_err(|e| CalcitErr::use_msg_stack(e, call_stack))?;
    return Ok(Some(v));
  }
  Ok(None)
}

pub fn run_fn(values: TernaryTreeList<Calcit>, info: &CalcitFn, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let mut body_scope = (*info.scope).to_owned();
  bind_args(&mut body_scope, &info.args, &values, call_stack)?;

  let v = evaluate_lines(&info.body, &body_scope, &info.def_ns, call_stack)?;

  if let Calcit::Recur(xs) = v {
    let mut current_values = xs;
    loop {
      bind_args(&mut body_scope, &info.args, &current_values, call_stack)?;
      let v = evaluate_lines(&info.body, &body_scope, &info.def_ns, call_stack)?;
      match v {
        Calcit::Recur(xs) => current_values = xs,
        result => return Ok(result),
      }
    }
  } else {
    Ok(v)
  }
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
pub fn bind_args(
  scope: &mut CalcitScope,
  args: &[CalcitArgLabel],
  values: &TernaryTreeList<Calcit>,
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
        CalcitArgLabel::Name(name) => {
          let mut chunk = CalcitList::new_inner();
          while let Some(v) = values.get(pop_values_idx.get_and_inc()) {
            chunk = chunk.push_right(v.to_owned());
          }
          scope.insert_mut(name.to_owned(), Calcit::List(Arc::new(chunk.into())));
          if pop_args_idx.0 < args.len() {
            return Err(CalcitErr::use_msg_stack(
              format!("extra args `{args:?}` after spreading in `{args:?}`",),
              call_stack,
            ));
          }
        }
        _ => {
          return Err(CalcitErr::use_msg_stack(
            format!("invalid control insode spreading mode: {args:?}"),
            call_stack,
          ));
        }
      }
    } else {
      match arg {
        CalcitArgLabel::RestMark => spreading = true,
        CalcitArgLabel::OptionalMark => optional = true,
        CalcitArgLabel::Name(name) => match values.get(pop_values_idx.get_and_inc()) {
          Some(v) => {
            scope.insert_mut(name.to_owned(), v.to_owned());
          }
          None => {
            if optional {
              scope.insert_mut(name.to_owned(), Calcit::Nil);
            } else {
              return Err(CalcitErr::use_msg_stack(
                format!("too few values `{}` passed to args `{args:?}`", values),
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
      format!(
        "extra args `{args:?}` not handled while passing values `{}` to args `{:?}`",
        CalcitList::from(values),
        args,
      ),
      call_stack,
    ))
  }
}

pub fn evaluate_lines(
  lines: &TernaryTreeList<Calcit>,
  scope: &CalcitScope,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let mut ret: Calcit = Calcit::Nil;
  for line in lines {
    match evaluate_expr(line, scope, file_ns, call_stack) {
      Ok(v) => ret = v,
      Err(e) => return Err(e),
    }
  }
  Ok(ret)
}

/// evaluate symbols before calling a function
/// notice that `&` is used to spread a list
pub fn evaluate_args(
  items: CalcitList,
  scope: &CalcitScope,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<TernaryTreeList<Calcit>, CalcitErr> {
  let mut ret: TernaryTreeList<Calcit> = TernaryTreeList::Empty;
  let mut spreading = false;
  for item in &items {
    match item {
      Calcit::Symbol { sym: s, .. } if &**s == "&" => {
        spreading = true;
      }
      _ => {
        if item.is_expr_evaluated() {
          if spreading {
            match item {
              Calcit::List(xs) => {
                for x in &**xs {
                  ret = ret.push((*x).to_owned());
                }
                spreading = false
              }
              a => {
                return Err(CalcitErr::use_msg_stack(
                  format!("expected list for spreading, got: {a}"),
                  call_stack,
                ))
              }
            }
          } else {
            ret = ret.push(item.to_owned());
          }
        } else {
          let v = evaluate_expr(item, scope, file_ns, call_stack)?;

          if spreading {
            match v {
              Calcit::List(xs) => {
                for x in &*xs {
                  ret = ret.push((*x).to_owned());
                }
                spreading = false
              }
              a => {
                return Err(CalcitErr::use_msg_stack(
                  format!("expected list for spreading, got: {a}"),
                  call_stack,
                ))
              }
            }
          } else {
            ret = ret.push(v.to_owned());
          }
        }
      }
    }
  }
  // println!("Evaluated args: {}", ret);
  Ok(ret)
}
