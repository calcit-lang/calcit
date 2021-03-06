pub mod preprocess;
pub mod track;

use im_ternary_tree::TernaryTreeList;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::builtins;
use crate::builtins::is_proc_name;
use crate::call_stack::{extend_call_stack, CallStackList, StackKind};
use crate::primes::{Calcit, CalcitErr, CalcitItems, CalcitScope, CalcitSyntax, CrListWrap, NodeLocation, SymbolResolved::*, CORE_NS};
use crate::program;
use crate::util::string::has_ns_part;

pub fn evaluate_expr(expr: &Calcit, scope: &CalcitScope, file_ns: Arc<str>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  // println!("eval code: {}", expr.lisp_str());

  match expr {
    Calcit::Nil => Ok(expr.to_owned()),
    Calcit::Bool(_) => Ok(expr.to_owned()),
    Calcit::Number(_) => Ok(expr.to_owned()),
    Calcit::Symbol { sym, .. } if &**sym == "&" => Ok(expr.to_owned()),
    Calcit::Symbol {
      sym,
      ns,
      at_def,
      resolved,
      location,
      ..
    } => {
      let loc = NodeLocation::new(ns.to_owned(), at_def.to_owned(), location.to_owned().unwrap_or_default());
      match resolved {
        Some(resolved_info) => match &*resolved_info.to_owned() {
          ResolvedDef {
            ns: r_ns,
            def: r_def,
            rule,
          } => {
            if rule.is_some() && sym != r_def {
              // dirty check for namespaced imported variables
              return eval_symbol_from_program(r_def, r_ns, call_stack);
            }
            evaluate_symbol(r_def, scope, r_ns, Some(loc), call_stack)
          }
          _ => evaluate_symbol(sym, scope, ns, Some(loc), call_stack),
        },
        _ => evaluate_symbol(sym, scope, ns, Some(loc), call_stack),
      }
    }
    Calcit::Keyword(_) => Ok(expr.to_owned()),
    Calcit::Str(_) => Ok(expr.to_owned()),
    Calcit::Thunk(code, v) => match v {
      None => evaluate_expr(code, scope, file_ns, call_stack),
      Some(data) => Ok((**data).to_owned()),
    },
    Calcit::Ref(_) => Ok(expr.to_owned()),
    Calcit::Tuple(..) => Ok(expr.to_owned()),
    Calcit::Buffer(..) => Ok(expr.to_owned()),
    Calcit::CirruQuote(..) => Ok(expr.to_owned()),
    Calcit::Recur(_) => unreachable!("recur not expected to be from symbol"),
    Calcit::List(xs) => match xs.get(0) {
      None => Err(CalcitErr::use_msg_stack(
        format!("cannot evaluate empty expr: {}", expr),
        call_stack,
      )),
      Some(x) => {
        // println!("eval expr: {}", expr.lisp_str());
        // println!("eval expr: {}", x);

        let v = evaluate_expr(x, scope, file_ns.to_owned(), call_stack)?;
        let rest_nodes = xs.drop_left();
        let ret = match &v {
          Calcit::Proc(p) => {
            let values = evaluate_args(&rest_nodes, scope, file_ns.to_owned(), call_stack)?;
            let next_stack = extend_call_stack(call_stack, file_ns, p.to_owned(), StackKind::Proc, Calcit::Nil, &values);

            if let Some(v) = p.strip_prefix('.') {
              builtins::meta::invoke_method(v, &values, &next_stack)
            } else {
              // println!("proc: {}", expr);
              builtins::handle_proc(p, &values, call_stack)
            }
          }
          Calcit::Syntax(s, def_ns) => {
            let next_stack = extend_call_stack(
              call_stack,
              def_ns.to_owned(),
              s.to_string().into(),
              StackKind::Syntax,
              expr.to_owned(),
              &rest_nodes,
            );

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
          Calcit::Fn {
            name,
            def_ns,
            scope: def_scope,
            args,
            body,
            ..
          } => {
            let values = evaluate_args(&rest_nodes, scope, file_ns, call_stack)?;
            let next_stack = extend_call_stack(
              call_stack,
              def_ns.to_owned(),
              name.to_owned(),
              StackKind::Fn,
              expr.to_owned(),
              &values,
            );

            run_fn(&values, def_scope, args, body, def_ns.to_owned(), &next_stack)
          }
          Calcit::Macro {
            name, def_ns, args, body, ..
          } => {
            println!(
              "[Warn] macro should already be handled during preprocessing: {}",
              Calcit::List(xs.to_owned()).lisp_str()
            );

            // TODO moving to preprocess
            let mut current_values = Box::new(rest_nodes.to_owned());
            // println!("eval macro: {} {}", x, expr.lisp_str()));
            // println!("macro... {} {}", x, CrListWrap(current_values.to_owned()));

            let next_stack = extend_call_stack(
              call_stack,
              def_ns.to_owned(),
              name.to_owned(),
              StackKind::Macro,
              expr.to_owned(),
              &rest_nodes,
            );

            Ok(loop {
              // need to handle recursion
              let body_scope = bind_args(args, &current_values, &CalcitScope::default(), call_stack)?;
              let code = evaluate_lines(body, &body_scope, def_ns.to_owned(), &next_stack)?;
              match code {
                Calcit::Recur(ys) => {
                  current_values = Box::new(ys.to_owned());
                }
                _ => {
                  // println!("gen code: {} {}", x, &code.lisp_str()));
                  break evaluate_expr(&code, scope, file_ns, &next_stack)?;
                }
              }
            })
          }
          Calcit::Keyword(k) => {
            if rest_nodes.len() == 1 {
              let v = evaluate_expr(&rest_nodes[0], scope, file_ns, call_stack)?;

              if let Calcit::Map(m) = v {
                match m.get(&Calcit::Keyword(k.to_owned())) {
                  Some(value) => Ok(value.to_owned()),
                  None => Ok(Calcit::Nil),
                }
              } else {
                Err(CalcitErr::use_msg_stack(format!("expected a hashmap, got {}", v), call_stack))
              }
            } else {
              Err(CalcitErr::use_msg_stack(
                format!("keyword only takes 1 argument, got: {}", CrListWrap(rest_nodes)),
                call_stack,
              ))
            }
          }
          Calcit::Symbol {
            sym,
            ns,
            at_def,
            resolved,
            location,
          } => {
            let error_location = location
              .as_ref()
              .map(|l| NodeLocation::new(ns.to_owned(), at_def.to_owned(), l.to_owned()));
            Err(CalcitErr::use_msg_stack_location(
              format!("cannot evaluate symbol directly: {}/{} in {}, {:?}", ns, sym, at_def, resolved),
              call_stack,
              error_location,
            ))
          }
          a => Err(CalcitErr::use_msg_stack_location(
            format!("cannot be used as operator: {} in {}", a, CrListWrap(xs.to_owned())),
            call_stack,
            a.get_location(),
          )),
        };

        ret
      }
    },
    Calcit::Set(_) => Err(CalcitErr::use_msg_stack("unexpected set for expr", call_stack)),
    Calcit::Map(_) => Err(CalcitErr::use_msg_stack("unexpected map for expr", call_stack)),
    Calcit::Record(..) => Err(CalcitErr::use_msg_stack("unexpected record for expr", call_stack)),
    Calcit::Proc(_) => Ok(expr.to_owned()),
    Calcit::Macro { .. } => Ok(expr.to_owned()),
    Calcit::Fn { .. } => Ok(expr.to_owned()),
    Calcit::Syntax(_, _) => Ok(expr.to_owned()),
  }
}

pub fn evaluate_symbol(
  sym: &str,
  scope: &CalcitScope,
  file_ns: &str,
  location: Option<NodeLocation>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let v = match parse_ns_def(sym) {
    Some((ns_part, def_part)) => match program::lookup_ns_target_in_import(file_ns.into(), &ns_part) {
      Some(target_ns) => match eval_symbol_from_program(&def_part, &target_ns, call_stack) {
        Ok(v) => Ok(v),
        Err(e) => Err(e),
      },
      None => Err(CalcitErr::use_msg_stack_location(
        format!("unknown ns target: {}/{}", ns_part, def_part),
        call_stack,
        location,
      )),
    },
    None => {
      if CalcitSyntax::is_valid(sym) {
        Ok(Calcit::Syntax(
          sym.try_into().map_err(|e| CalcitErr::use_msg_stack(e, call_stack))?,
          file_ns.to_owned().into(),
        ))
      } else if let Some(v) = scope.get(sym) {
        // although scope is detected first, it would trigger warning during preprocess
        Ok(v.to_owned())
      } else if is_proc_name(sym) {
        Ok(Calcit::Proc(sym.to_owned().into()))
      } else if program::lookup_def_code(CORE_NS, sym).is_some() {
        eval_symbol_from_program(sym, CORE_NS, call_stack)
      } else if program::has_def_code(file_ns, sym) {
        eval_symbol_from_program(sym, file_ns, call_stack)
      } else {
        match program::lookup_def_target_in_import(file_ns, sym) {
          Some(target_ns) => eval_symbol_from_program(sym, &target_ns, call_stack),
          None => {
            let vars = scope.list_variables();
            Err(CalcitErr::use_msg_stack_location(
              format!("unknown symbol `{}` in {}", sym, vars.join(",")),
              call_stack,
              location,
            ))
          }
        }
      }
    }
  }?;
  match v {
    Calcit::Thunk(_code, Some(data)) => Ok((*data).to_owned()),
    // extra check to make sure code in thunks evaluated
    Calcit::Thunk(code, None) => evaluate_def_thunk(&code, file_ns, sym, call_stack),
    _ => Ok(v),
  }
}

/// make sure a thunk at global is called
pub fn evaluate_def_thunk(code: &Arc<Calcit>, file_ns: &str, sym: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let evaled_v = evaluate_expr(code, &CalcitScope::default(), file_ns.into(), call_stack)?;
  // and write back to program state to fix duplicated evalution
  // still using thunk since js and IR requires bare code
  let next = if builtins::effects::is_rust_eval() {
    // no longer useful for evaling
    Arc::new(Calcit::Nil)
  } else {
    code.to_owned()
  };
  program::write_evaled_def(file_ns, sym, Calcit::Thunk(next, Some(Arc::new(evaled_v.to_owned()))))?;
  Ok(evaled_v)
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

fn eval_symbol_from_program(sym: &str, ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  match program::lookup_evaled_def(ns, sym) {
    Some(v) => Ok(v),
    None => match program::lookup_def_code(ns, sym) {
      Some(code) => {
        let v = evaluate_expr(&code, &CalcitScope::default(), ns.into(), call_stack)?;
        program::write_evaled_def(ns, sym, v.to_owned()).map_err(|e| CalcitErr::use_msg_stack(e, call_stack))?;
        Ok(v)
      }
      None => Err(CalcitErr::use_msg_stack(
        format!("cannot find code for def: {}/{}", ns, sym),
        call_stack,
      )),
    },
  }
}

pub fn run_fn(
  values: &CalcitItems,
  scope: &CalcitScope,
  args: &[Arc<str>],
  body: &CalcitItems,
  file_ns: Arc<str>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let mut current_values = Box::new(values.to_owned());
  loop {
    let body_scope = bind_args(args, &current_values, scope, call_stack)?;
    let v = evaluate_lines(body, &body_scope, file_ns.to_owned(), call_stack)?;
    match v {
      Calcit::Recur(xs) => {
        current_values = Box::new(xs.to_owned());
      }
      result => return Ok(result),
    }
  }
}

/// create new scope by writing new args
/// notice that `&` is a mark for spreading, `?` for optional arguments
pub fn bind_args(
  args: &[Arc<str>],
  values: &CalcitItems,
  base_scope: &CalcitScope,
  call_stack: &CallStackList,
) -> Result<CalcitScope, CalcitErr> {
  let mut scope = base_scope.to_owned();
  let mut spreading = false;
  let mut optional = false;

  let collected_args = args.to_owned();
  let collected_values = Arc::new(values);
  let pop_args_idx = Rc::new(RefCell::new(0));
  let pop_values_idx = Rc::new(RefCell::new(0));

  let args_pop_front = || -> Option<Arc<str>> {
    let mut p = pop_args_idx.borrow_mut();
    let ret = collected_args.get(*p);
    *p += 1;
    ret.map(|x| x.to_owned())
  };

  let values_pop_front = || -> Option<&Calcit> {
    let mut p = pop_values_idx.borrow_mut();
    let ret = collected_values.get(*p);
    *p += 1;
    ret
  };
  let is_args_empty = || -> bool {
    let p = pop_args_idx.borrow();
    *p >= (*collected_args).len()
  };
  let is_values_empty = || -> bool {
    let p = pop_values_idx.borrow();
    *p >= (*collected_values).len()
  };

  while let Some(sym) = args_pop_front() {
    if spreading {
      match &*sym {
        "&" => return Err(CalcitErr::use_msg_stack(format!("invalid & in args: {:?}", args), call_stack)),
        "?" => return Err(CalcitErr::use_msg_stack(format!("invalid ? in args: {:?}", args), call_stack)),
        _ => {
          let mut chunk: CalcitItems = TernaryTreeList::Empty;
          while let Some(v) = values_pop_front() {
            chunk = chunk.push_right(v.to_owned());
          }
          scope.insert(sym.to_owned(), Calcit::List(chunk));
          if !is_args_empty() {
            return Err(CalcitErr::use_msg_stack(
              format!("extra args `{:?}` after spreading in `{:?}`", collected_args, args,),
              call_stack,
            ));
          }
        }
      }
    } else {
      match &*sym {
        "&" => spreading = true,
        "?" => optional = true,
        _ => match values_pop_front() {
          Some(v) => {
            scope.insert(sym.to_owned(), v.to_owned());
          }
          None => {
            if optional {
              scope.insert(sym.to_owned(), Calcit::Nil);
            } else {
              return Err(CalcitErr::use_msg_stack(
                format!("too few values `{}` passed to args `{:?}`", CrListWrap(values.to_owned()), args),
                call_stack,
              ));
            }
          }
        },
      }
    }
  }
  if is_values_empty() {
    Ok(scope)
  } else {
    Err(CalcitErr::use_msg_stack(
      format!(
        "extra args `{}` not handled while passing values `{}` to args `{:?}`",
        CrListWrap((*collected_values).to_owned()),
        CrListWrap(values.to_owned()),
        args,
      ),
      call_stack,
    ))
  }
}

pub fn evaluate_lines(
  lines: &CalcitItems,
  scope: &CalcitScope,
  file_ns: Arc<str>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let mut ret: Calcit = Calcit::Nil;
  for line in lines {
    match evaluate_expr(line, scope, file_ns.to_owned(), call_stack) {
      Ok(v) => ret = v,
      Err(e) => return Err(e),
    }
  }
  Ok(ret)
}

/// evaluate symbols before calling a function
/// notice that `&` is used to spread a list
pub fn evaluate_args(
  items: &CalcitItems,
  scope: &CalcitScope,
  file_ns: Arc<str>,
  call_stack: &CallStackList,
) -> Result<CalcitItems, CalcitErr> {
  let mut ret: Vec<Calcit> = Vec::with_capacity(items.len());
  let mut spreading = false;
  for item in items {
    match item {
      Calcit::Symbol { sym: s, .. } if &**s == "&" => {
        spreading = true;
      }
      _ => {
        let v = evaluate_expr(item, scope, file_ns.to_owned(), call_stack)?;

        if spreading {
          match v {
            Calcit::List(xs) => {
              for x in &xs {
                // extract thunk before calling functions
                let y = match x {
                  Calcit::Thunk(code, v) => match v {
                    None => evaluate_expr(code, scope, file_ns.to_owned(), call_stack)?,
                    Some(data) => (**data).to_owned(),
                  },
                  _ => x.to_owned(),
                };
                ret.push(y.to_owned());
              }
              spreading = false
            }
            a => {
              return Err(CalcitErr::use_msg_stack(
                format!("expected list for spreading, got: {}", a),
                call_stack,
              ))
            }
          }
        } else {
          // extract thunk before calling functions
          let y = match v {
            Calcit::Thunk(code, value) => match value {
              None => evaluate_expr(&code, scope, file_ns.to_owned(), call_stack)?,
              Some(data) => (*data).to_owned(),
            },
            _ => v.to_owned(),
          };
          ret.push(y);
        }
      }
    }
  }
  Ok(TernaryTreeList::from(&ret))
}
