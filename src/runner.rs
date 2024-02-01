pub mod preprocess;
pub mod track;

use im_ternary_tree::TernaryTreeList;
use std::sync::Arc;
use strum::ParseError;

use crate::builtins::{self, is_registered_proc, IMPORTED_PROCS};
use crate::call_stack::{extend_call_stack, CallStackList, StackKind};
use crate::primes::{
  Calcit, CalcitErr, CalcitFn, CalcitItems, CalcitProc, CalcitScope, CalcitSymbolInfo, CalcitSyntax, CrListWrap, MethodKind,
  NodeLocation, SymbolResolved::*, CORE_NS,
};
use crate::program;
use crate::util::string::has_ns_part;

pub fn evaluate_expr(expr: &Calcit, scope: &CalcitScope, file_ns: Arc<str>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  // println!("eval code: {}", expr.lisp_str());

  match expr {
    Calcit::Nil => Ok(expr.to_owned()),
    Calcit::Bool(_) => Ok(expr.to_owned()),
    Calcit::Number(_) => Ok(expr.to_owned()),
    Calcit::Symbol { sym, .. } if &**sym == "&" => Ok(expr.to_owned()),
    Calcit::Symbol { sym, info, location, .. } => {
      let loc = NodeLocation::new(info.ns.to_owned(), info.at_def.to_owned(), location.to_owned().unwrap_or_default());
      match &info.resolved {
        Some(ResolvedDef {
          ns: r_ns,
          def: r_def,
          rule,
        }) => {
          if rule.is_some() && sym != r_def {
            // dirty check for namespaced imported variables
            return eval_symbol_from_program(r_def, r_ns, call_stack);
          }
          evaluate_symbol(r_def, scope, r_ns, Some(loc), call_stack)
        }
        _ => evaluate_symbol(sym, scope, &info.ns, Some(loc), call_stack),
      }
    }
    Calcit::Tag(_) => Ok(expr.to_owned()),
    Calcit::Str(_) => Ok(expr.to_owned()),
    Calcit::Thunk(code, v) => match v {
      None => evaluate_expr(code, scope, file_ns, call_stack),
      Some(data) => Ok((**data).to_owned()),
    },
    Calcit::Ref(..) => Ok(expr.to_owned()),
    Calcit::Tuple(..) => Ok(expr.to_owned()),
    Calcit::Buffer(..) => Ok(expr.to_owned()),
    Calcit::CirruQuote(..) => Ok(expr.to_owned()),
    Calcit::Recur(_) => unreachable!("recur not expected to be from symbol"),
    Calcit::RawCode(_, code) => unreachable!("raw code `{}` cannot be called", code),
    Calcit::List(xs) => match xs.get(0) {
      None => Err(CalcitErr::use_msg_stack(format!("cannot evaluate empty expr: {expr}"), call_stack)),
      Some(x) => {
        // println!("eval expr: {}", expr.lisp_str());
        // println!("eval expr: {}", x);

        let v = evaluate_expr(x, scope, file_ns.to_owned(), call_stack)?;
        let rest_nodes = xs.drop_left();
        let ret = match &v {
          Calcit::Proc(p) => {
            let values = evaluate_args(&rest_nodes, scope, file_ns, call_stack)?;
            builtins::handle_proc(*p, &values, call_stack)
          }
          Calcit::Syntax(s, def_ns) => {
            let next_stack = extend_call_stack(
              call_stack,
              def_ns.to_owned(),
              Arc::from(s.as_ref()),
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
          Calcit::Method(name, kind) => {
            let values = evaluate_args(&rest_nodes, scope, file_ns.to_owned(), call_stack)?;
            let next_stack = extend_call_stack(call_stack, file_ns, name.to_owned(), StackKind::Method, Calcit::Nil, &values);

            if *kind == MethodKind::Invoke {
              builtins::meta::invoke_method(name, &values, &next_stack)
            } else {
              CalcitErr::err_str(format!("unknown method for rust runtime: {kind}"))
            }
          }
          Calcit::Fn { info, .. } => {
            let values = evaluate_args(&rest_nodes, scope, file_ns, call_stack)?;
            let next_stack = extend_call_stack(
              call_stack,
              info.def_ns.to_owned(),
              info.name.to_owned(),
              StackKind::Fn,
              expr.to_owned(),
              &values,
            );

            run_fn(&values, info, &next_stack)
          }
          Calcit::Macro { info, .. } => {
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
              info.def_ns.to_owned(),
              info.name.to_owned(),
              StackKind::Macro,
              expr.to_owned(),
              &rest_nodes,
            );

            let mut body_scope = CalcitScope::default();

            Ok(loop {
              // need to handle recursion
              bind_args(&mut body_scope, &info.args, &current_values, call_stack)?;
              let code = evaluate_lines(&info.body, &body_scope, info.def_ns.to_owned(), &next_stack)?;
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
                format!("tag only takes 1 argument, got: {}", CrListWrap(rest_nodes)),
                call_stack,
              ))
            }
          }
          Calcit::Symbol { sym, info, location } => {
            let ps = IMPORTED_PROCS.read().expect("read procs");
            let name = &*sym.to_owned();
            match ps.get(name) {
              Some(f) => {
                let values = evaluate_args(&rest_nodes, scope, file_ns, call_stack)?;
                f(&values, call_stack)
              }
              None => {
                let error_location = location
                  .as_ref()
                  .map(|l| NodeLocation::new(info.ns.to_owned(), info.at_def.to_owned(), l.to_owned()));
                let ns = &info.ns;
                let at_def = &info.at_def;
                let resolved = &info.resolved;
                Err(CalcitErr::use_msg_stack_location(
                  format!("cannot evaluate symbol directly: {ns}/{sym} in {at_def}, {resolved:?}"),
                  call_stack,
                  error_location,
                ))
              }
            }
          }
          a => Err(CalcitErr::use_msg_stack_location(
            format!("cannot be used as operator: {a} in {}", CrListWrap(xs.to_owned())),
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
    Calcit::Method(..) => Ok(expr.to_owned()),
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
    Some((ns_part, def_part)) => match program::lookup_ns_target_in_import(file_ns, &ns_part) {
      Some(target_ns) => match eval_symbol_from_program(&def_part, &target_ns, call_stack) {
        Ok(v) => Ok(v),
        Err(e) => Err(e),
      },
      None => Err(CalcitErr::use_msg_stack_location(
        format!("unknown ns target: {ns_part}/{def_part}"),
        call_stack,
        location,
      )),
    },
    None => {
      if CalcitSyntax::is_valid(sym) {
        Ok(Calcit::Syntax(
          sym
            .parse()
            .map_err(|e: ParseError| CalcitErr::use_msg_stack(format!("{} {}", sym, e), call_stack))?,
          file_ns.into(),
        ))
      } else if let Some(v) = scope.get(sym) {
        // although scope is detected first, it would trigger warning during preprocess
        Ok(v.to_owned())
      } else if let Ok(p) = sym.parse::<CalcitProc>() {
        Ok(Calcit::Proc(p))
      } else if is_registered_proc(sym) {
        Ok(Calcit::Symbol {
          sym: sym.into(),
          info: Arc::new(CalcitSymbolInfo {
            ns: file_ns.into(),
            at_def: file_ns.into(),
            resolved: None,
          }),
          location: location.map(|x| x.coord),
        })
      } else if program::lookup_def_code(CORE_NS, sym).is_some() {
        eval_symbol_from_program(sym, CORE_NS, call_stack)
      } else if program::has_def_code(file_ns, sym) {
        eval_symbol_from_program(sym, file_ns, call_stack)
      } else {
        match program::lookup_def_target_in_import(file_ns, sym) {
          Some(target_ns) => eval_symbol_from_program(sym, &target_ns, call_stack),
          None => {
            let mut vars = String::new();
            for (i, k) in scope.0.keys().enumerate() {
              if i > 0 {
                vars.push(',');
              }
              vars.push_str(k);
            }
            Err(CalcitErr::use_msg_stack_location(
              format!("unknown symbol `{sym}` in {}", vars),
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
        format!("cannot find code for def: {ns}/{sym}"),
        call_stack,
      )),
    },
  }
}

pub fn run_fn(values: &CalcitItems, info: &CalcitFn, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let mut body_scope = (*info.scope).to_owned();
  bind_args(&mut body_scope, &info.args, values, call_stack)?;

  let v = evaluate_lines(&info.body, &body_scope, info.def_ns.to_owned(), call_stack)?;

  if let Calcit::Recur(xs) = v {
    let mut current_values = xs;
    loop {
      bind_args(&mut body_scope, &info.args, &current_values, call_stack)?;
      let v = evaluate_lines(&info.body, &body_scope, info.def_ns.to_owned(), call_stack)?;
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
  args: &[Arc<str>],
  values: &CalcitItems,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  let mut spreading = false;
  let mut optional = false;

  let mut pop_args_idx = MutIndex::default();
  let mut pop_values_idx = MutIndex::default();

  while let Some(sym) = args.get(pop_args_idx.get_and_inc()) {
    if spreading {
      match &**sym {
        "&" => return Err(CalcitErr::use_msg_stack(format!("invalid & in args: {args:?}"), call_stack)),
        "?" => return Err(CalcitErr::use_msg_stack(format!("invalid ? in args: {args:?}"), call_stack)),
        _ => {
          let mut chunk: CalcitItems = TernaryTreeList::Empty;
          while let Some(v) = values.get(pop_values_idx.get_and_inc()) {
            chunk = chunk.push_right(v.to_owned());
          }
          scope.insert_mut(sym.to_owned(), Calcit::List(chunk));
          if pop_args_idx.0 < args.len() {
            return Err(CalcitErr::use_msg_stack(
              format!("extra args `{args:?}` after spreading in `{args:?}`",),
              call_stack,
            ));
          }
        }
      }
    } else {
      match &**sym {
        "&" => spreading = true,
        "?" => optional = true,
        _ => match values.get(pop_values_idx.get_and_inc()) {
          Some(v) => {
            scope.insert_mut(sym.to_owned(), v.to_owned());
          }
          None => {
            if optional {
              scope.insert_mut(sym.to_owned(), Calcit::Nil);
            } else {
              return Err(CalcitErr::use_msg_stack(
                format!("too few values `{}` passed to args `{args:?}`", CrListWrap(values.to_owned())),
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
        "extra args `{}` not handled while passing values `{}` to args `{:?}`",
        CrListWrap((*values).to_owned()),
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
  let mut ret: TernaryTreeList<Calcit> = TernaryTreeList::Empty;
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
                ret = ret.push(y.to_owned());
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
          // extract thunk before calling functions
          let y = match v {
            Calcit::Thunk(code, value) => match value {
              None => evaluate_expr(&code, scope, file_ns.to_owned(), call_stack)?,
              Some(data) => (*data).to_owned(),
            },
            _ => v.to_owned(),
          };
          ret = ret.push(y);
        }
      }
    }
  }
  Ok(ret)
}
