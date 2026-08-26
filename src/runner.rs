pub mod macro_cache;
pub mod macro_capability;
pub mod macro_metrics;
pub mod preprocess;
pub mod track;

use std::cell::RefCell;
use std::sync::Arc;
use std::vec;

use crate::builtins;
use crate::calcit::{
  CORE_NS, Calcit, CalcitArgLabel, CalcitCallKind, CalcitEnumValue, CalcitErr, CalcitErrKind, CalcitFn, CalcitFnArgs, CalcitImport,
  CalcitList, CalcitListView, CalcitLocal, CalcitNumberBinaryOp, CalcitProc, CalcitScope, CalcitSyntax, MethodKind, NodeLocation,
  trailing_option_arg_count,
};
use crate::call_stack::{CallStackList, StackKind, using_stack};
use crate::data::cirru;
use crate::program;
use crate::util::string::has_ns_part;
use cirru_edn::EdnTag;

fn build_runtime_cell_error(ns: &str, def: &str, call_stack: &CallStackList, cell: program::RuntimeCell) -> CalcitErr {
  match cell {
    program::RuntimeCell::Resolving => CalcitErr::use_msg_stack(
      CalcitErrKind::Unexpected,
      format!("definition is still resolving: {ns}/{def}"),
      call_stack,
    ),
    program::RuntimeCell::Errored(message) => CalcitErr::use_msg_stack(
      CalcitErrKind::Unexpected,
      format!("definition is in errored state: {ns}/{def}\n{message}"),
      call_stack,
    ),
    program::RuntimeCell::Cold | program::RuntimeCell::Lazy { .. } | program::RuntimeCell::Ready(_) => CalcitErr::use_msg_stack(
      CalcitErrKind::Unexpected,
      format!("unexpected runtime state for {ns}/{def}"),
      call_stack,
    ),
  }
}

fn require_symbol_from_program(sym: &str, ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  eval_symbol_from_program(sym, ns, call_stack).map(|value| value.expect("value"))
}

fn lookup_symbol_in_program_namespaces(sym: &str, file_ns: &str, call_stack: &CallStackList) -> Result<Option<Calcit>, CalcitErr> {
  if let Some(value) = eval_symbol_from_program(sym, CORE_NS, call_stack)? {
    Ok(Some(value))
  } else if let Some(value) = eval_symbol_from_program(sym, file_ns, call_stack)? {
    Ok(Some(value))
  } else {
    Ok(None)
  }
}

fn resolve_runtime_or_compiled_def(
  ns: &str,
  def: &str,
  def_id: Option<program::DefId>,
  call_stack: &CallStackList,
) -> Result<Option<Calcit>, CalcitErr> {
  program::resolve_runtime_or_compiled_def(ns, def, def_id, program::RuntimeResolveMode::Strict, call_stack).map_err(|err| match err {
    program::RuntimeResolveError::RuntimeCell(cell) => build_runtime_cell_error(ns, def, call_stack, cell),
    program::RuntimeResolveError::Eval(failure) => failure,
    program::RuntimeResolveError::RuntimeSeed(message) => CalcitErr::use_msg_stack(
      CalcitErrKind::Unexpected,
      format!("failed to seed runtime for {ns}/{def}: {message}"),
      call_stack,
    ),
  })
}

fn format_fn_arg_labels(args: &CalcitFnArgs) -> String {
  match args {
    CalcitFnArgs::Args(xs) => xs
      .iter()
      .map(|idx| CalcitLocal::read_name(*idx).to_string())
      .collect::<Vec<_>>()
      .join(" "),
    CalcitFnArgs::MarkedArgs(xs) => xs.iter().map(ToString::to_string).collect::<Vec<_>>().join(" "),
  }
}

fn format_runtime_values(values: &[Calcit]) -> String {
  if values.is_empty() {
    "[]".to_owned()
  } else {
    format!("{}", CalcitList::from(values))
  }
}

fn build_fn_arity_mismatch_error(info: &CalcitFn, values: &[Calcit], call_stack: &CallStackList, phase: &str) -> CalcitErr {
  let expected = info.args.param_len();
  let actual = values.len();
  let def_ref = info
    .def_ref
    .as_ref()
    .map(|r| format!("{}/{}", r.def_ns, r.def_name))
    .unwrap_or_else(|| format!("{}/{}", info.def_ns, info.name));
  let msg = format!(
    "function arity mismatch during {phase}: `{}` expected {expected} argument(s), got {actual}\n  params: ({})\n  values: {}\n  fn-namespace: {}\n  fn-name: {}\n  def-ref: {}",
    info.name,
    format_fn_arg_labels(info.args.as_ref()),
    format_runtime_values(values),
    info.def_ns,
    info.name,
    def_ref,
  );
  CalcitErr::use_msg_stack(CalcitErrKind::Unexpected, msg, call_stack)
}

fn number_binary_proc(operation: CalcitNumberBinaryOp) -> CalcitProc {
  match operation {
    CalcitNumberBinaryOp::Add => CalcitProc::NativeAdd,
    CalcitNumberBinaryOp::Subtract => CalcitProc::NativeMinus,
    CalcitNumberBinaryOp::Multiply => CalcitProc::NativeMultiply,
    CalcitNumberBinaryOp::Divide => CalcitProc::NativeDivide,
    CalcitNumberBinaryOp::Remainder => CalcitProc::NativeNumberRem,
    CalcitNumberBinaryOp::LessThan => CalcitProc::NativeLessThan,
    CalcitNumberBinaryOp::GreaterThan => CalcitProc::NativeGreaterThan,
  }
}

fn evaluate_number_binary_call(
  operation: CalcitNumberBinaryOp,
  xs: &CalcitList,
  scope: &CalcitScope,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  debug_assert_eq!(xs.len(), 3, "specialized binary calls must contain a head and two arguments");

  let evaluate_arg = |arg: &Calcit| {
    if arg.is_expr_evaluated() {
      Ok(arg.to_owned())
    } else {
      evaluate_expr(arg, scope, file_ns, call_stack)
    }
  };
  // Preserve normal call semantics: each argument is evaluated exactly once,
  // from left to right, before the operator sees either value.
  let values = [evaluate_arg(&xs[1])?, evaluate_arg(&xs[2])?];

  let result = match (&values[0], &values[1]) {
    (Calcit::Number(a), Calcit::Number(b)) => match operation {
      CalcitNumberBinaryOp::Add => Ok(Calcit::Number(a + b)),
      CalcitNumberBinaryOp::Subtract => Ok(Calcit::Number(a - b)),
      CalcitNumberBinaryOp::Multiply => Ok(Calcit::Number(a * b)),
      CalcitNumberBinaryOp::Divide => Ok(Calcit::Number(a / b)),
      CalcitNumberBinaryOp::Remainder => builtins::rem_numbers(*a, *b),
      CalcitNumberBinaryOp::LessThan => Ok(Calcit::Bool(a < b)),
      CalcitNumberBinaryOp::GreaterThan => Ok(Calcit::Bool(a > b)),
    },
    // Static evidence may become stale after hot reload or host interop. Keep
    // the established dynamic error and stack behavior without re-evaluating.
    _ => builtins::handle_proc(number_binary_proc(operation), &values, call_stack),
  };

  if using_stack() {
    result.map_err(|err| {
      if err.stack.is_empty() {
        let mut stacked = err;
        call_stack.clone_into(&mut stacked.stack);
        stacked
      } else {
        err
      }
    })
  } else {
    result
  }
}

pub fn evaluate_expr(expr: &Calcit, scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  // println!("eval code: {}", expr.lisp_str());
  use Calcit::*;

  match expr {
    Nil
    | Unit
    | Bool(_)
    | Number(_)
    | Registered(_)
    | Tag(_)
    | Str(_)
    | Ref(..)
    | Enum { .. }
    | Buffer(..)
    | BufList(..)
    | CirruQuote(..)
    | Proc(_)
    | Macro { .. }
    | Fn { .. }
    | StructDef { .. }
    | EnumDef { .. }
    | Trait { .. }
    | Impl { .. }
    | Syntax(_, _)
    | Method(..)
    | AnyRef(..) => Ok(expr.to_owned()),

    Thunk(thunk) => Ok(thunk.evaluated(scope, call_stack)?),
    Symbol { sym, info, location, .. } => {
      // println!("[Warn] slow path reading symbol: {}", sym);
      evaluate_symbol(sym, scope, &info.at_ns, &info.at_def, location, call_stack)
    }
    Local(CalcitLocal { idx, sym, .. }) => evaluate_symbol_from_scope(*idx, sym, scope),
    Import(CalcitImport { ns, def, def_id, .. }) => evaluate_symbol_from_program(def, ns, *def_id, call_stack),
    List(xs) => match xs.first() {
      None => Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Arity,
        format!("cannot evaluate empty expr: {expr}"),
        call_stack,
        expr.get_location(),
      )),
      Some(x) => {
        if let CalcitCallKind::NumberBinary(operation) = xs.call_kind()
          && xs.len() == 3
          && matches!(xs.first(), Some(Calcit::Proc(proc)) if *proc == number_binary_proc(operation))
        {
          return evaluate_number_binary_call(operation, xs, scope, file_ns, call_stack);
        }
        // println!("eval expr: {}", expr.lisp_str());
        // println!("eval expr x: {}", x);

        let call = xs.view();
        if x.is_expr_evaluated() {
          call_expr(x, &call, scope, file_ns, call_stack, false)
        } else {
          let v = evaluate_expr(x, scope, file_ns, call_stack)?;
          call_expr(&v, &call, scope, file_ns, call_stack, false)
        }
      }
    },
    Recur(_) => unreachable!("recur not expected to be from symbol"),
    RawCode(_, code) => {
      macro_capability::check_host_ffi(code, call_stack)?;
      Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Unexpected,
        format!("raw host code `{code}` cannot be evaluated by the native runtime"),
        call_stack,
        expr.get_location(),
      ))
    }
    Set(_) => Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Unexpected,
      "unexpected set for expr",
      call_stack,
      expr.get_location(),
    )),
    Map(_) => Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Unexpected,
      "unexpected map for expr",
      call_stack,
      expr.get_location(),
    )),
    Struct { .. } => Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Unexpected,
      "unexpected struct value for expr",
      call_stack,
      expr.get_location(),
    )),
  }
}

pub fn call_expr(
  v: &Calcit,
  xs: &CalcitListView<'_>,
  scope: &CalcitScope,
  file_ns: &str,
  call_stack: &CallStackList,
  spreading: bool,
) -> Result<Calcit, CalcitErr> {
  // println!("calling expr: {}", xs);
  match v {
    Calcit::Proc(p) => {
      let values = if spreading {
        evaluate_spreaded_args_from(xs, 1, scope, file_ns, call_stack)?
      } else {
        evaluate_args_from(xs, 1, scope, file_ns, call_stack)?
      };
      builtins::handle_proc(*p, &values, call_stack)
    }
    Calcit::Syntax(s, def_ns) => {
      macro_capability::check_syntax(s, call_stack)?;
      let rest_nodes = xs.skip(1).expect("expected syntax rest nodes");
      if using_stack() {
        let next_stack = call_stack.extend_owned(
          def_ns,
          s.as_ref(),
          StackKind::Syntax,
          Calcit::from(xs.to_vec()),
          rest_nodes.to_vec(),
        );
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
      macro_capability::check_method(name, kind, call_stack)?;
      if matches!(kind, MethodKind::Invoke(_)) {
        let values = if spreading {
          evaluate_spreaded_args_from(xs, 1, scope, file_ns, call_stack)?
        } else {
          evaluate_args_from(xs, 1, scope, file_ns, call_stack)?
        };
        if using_stack() {
          let next_stack = call_stack.extend(file_ns, name, StackKind::Method, &Calcit::Nil, &values);
          builtins::meta::invoke_method(name, &values, &next_stack)
        } else {
          builtins::meta::invoke_method(name, &values, call_stack)
        }
      } else if matches!(kind, MethodKind::TagAccess) {
        if xs.len() == 2 {
          let obj = evaluate_expr(&xs[1], scope, file_ns, call_stack)?;
          let tag = evaluate_expr(&Calcit::tag(name), scope, file_ns, call_stack)?;
          if let Calcit::Map(m) = obj {
            match m.get(&tag) {
              Some(value) => Ok(value.to_owned()),
              None => Ok(Calcit::Nil),
            }
          } else {
            Err(CalcitErr::use_msg_stack_location(
              CalcitErrKind::Type,
              format!("expected a hashmap, got: {obj}"),
              call_stack,
              obj.get_location(),
            ))
          }
        } else {
          Err(CalcitErr::use_msg_stack_location(
            CalcitErrKind::Arity,
            format!("tag-accessor takes only 1 argument, {xs}"),
            call_stack,
            xs.first().and_then(|node| node.get_location()),
          ))
        }
      } else {
        Err(CalcitErr::use_msg_stack_location(
          CalcitErrKind::Unexpected,
          format!(
            "method kind `{kind}` (`.{prefix}{name}`) is only available in JS codegen, not supported in Rust runtime. \
             Use `calcit js` to compile to JS, or avoid `.!` / `.-` syntax in server-side code. \
             Expression: `{xs}`",
            prefix = match kind {
              MethodKind::InvokeNative => "!",
              MethodKind::InvokeNativeOptional => "?!",
              MethodKind::Access => "-",
              MethodKind::AccessOptional => "?-",
              _ => "?",
            },
          ),
          call_stack,
          xs.first().and_then(|node| node.get_location()),
        ))
      }
    }
    Calcit::Fn { info, .. } => {
      let values = if spreading {
        evaluate_spreaded_args_from(xs, 1, scope, file_ns, call_stack)?
      } else {
        evaluate_args_from(xs, 1, scope, file_ns, call_stack)?
      };
      if using_stack() {
        let next_stack = call_stack.extend(&info.def_ns, &info.name, StackKind::Fn, &Calcit::from(xs.to_vec()), &values);
        run_fn_owned(values, info, &next_stack)
      } else {
        run_fn_owned(values, info, call_stack)
      }
    }
    Calcit::Macro { info, .. } => {
      eprintln!(
        "[Warn] macro should already be handled during preprocessing: {}",
        Calcit::from(xs.to_vec()).lisp_str()
      );

      let mut current_values: Vec<Calcit> = xs.iter().skip(1).cloned().collect();
      let macro_name = format!("{}/{}", info.def_ns, info.name);
      macro_metrics::record_expansion(&macro_name, info.signature.as_ref());
      macro_metrics::record_cache_bypass(&macro_name, "runtime-evaluator");

      let next_stack = if using_stack() {
        call_stack.extend_owned(
          &info.def_ns,
          &info.name,
          StackKind::Macro,
          Calcit::from(xs.to_vec()),
          current_values.clone(),
        )
      } else {
        call_stack.to_owned()
      };

      // TODO moving to preprocess
      // println!("eval macro: {} {}", x, expr.lisp_str()));
      // println!("macro... {} {}", x, CrListWrap(current_values.to_owned()));

      let mut body_scope = CalcitScope::default();
      let frame_checkpoint = body_scope.frame_checkpoint();

      Ok(loop {
        // need to handle recursion
        body_scope.restore_frame(frame_checkpoint);
        bind_marked_args(&mut body_scope, &info.args, &current_values, call_stack)?;
        let evaluate_body = || {
          let _timer = macro_metrics::PhaseTimer::start(&macro_name, macro_metrics::MacroMetricPhase::Evaluator);
          evaluate_lines(info.body.as_ref().as_slice(), &body_scope, &info.def_ns, &next_stack)
        };
        let code = if info.signature.is_strict() {
          macro_capability::with_macro_context(
            Arc::from(format!("{}/{}", info.def_ns, info.name)),
            info.signature.capabilities.clone(),
            xs.first().and_then(Calcit::get_location),
            evaluate_body,
          )?
        } else {
          evaluate_body()?
        };
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
      if xs.len() == 2 {
        let v = evaluate_expr(&xs[1], scope, file_ns, call_stack)?;

        match &v {
          Calcit::Map(m) => match m.get(&Calcit::Tag(k.to_owned())) {
            Some(value) => Ok(value.to_owned()),
            None => Ok(Calcit::Nil),
          },
          Calcit::Struct(struct_value) => struct_value.get(k.ref_str()).cloned().ok_or_else(|| {
            CalcitErr::use_msg_stack_location(
              CalcitErrKind::Type,
              format!("struct `{}` does not define field `:{k}`", struct_value.struct_ref.name),
              call_stack,
              v.get_location(),
            )
          }),
          _ => Err(CalcitErr::use_msg_stack_location(
            CalcitErrKind::Type,
            format!("expected a hashmap or struct, got: {v}"),
            call_stack,
            v.get_location(),
          )),
        }
      } else {
        Err(CalcitErr::use_msg_stack_location(
          CalcitErrKind::Arity,
          format!("tag only takes 1 argument, got: {}", xs.len().saturating_sub(1)),
          call_stack,
          xs.first().and_then(|node| node.get_location()),
        ))
      }
    }
    Calcit::Registered(alias) => {
      macro_capability::check_registered(alias, call_stack)?;
      let values = if spreading {
        evaluate_spreaded_args_from(xs, 1, scope, file_ns, call_stack)?
      } else {
        evaluate_args_from(xs, 1, scope, file_ns, call_stack)?
      };
      builtins::call_registered_proc(alias, values, call_stack).map_err(|e| {
        if e.kind == CalcitErrKind::Var {
          CalcitErr::use_msg_stack_location(
            CalcitErrKind::Var,
            format!("cannot evaluate symbol directly: {file_ns}/{alias}"),
            call_stack,
            xs.first().and_then(|node| node.get_location()),
          )
        } else {
          e
        }
      })
    }
    a => {
      let location = xs
        .first()
        .and_then(|node| node.get_location())
        .or_else(|| a.get_location())
        .or_else(|| xs.get(1).and_then(|node| node.get_location()));
      let expr_one_liner = {
        let expr = Calcit::from(xs.to_vec());
        match cirru::calcit_to_cirru(&expr) {
          Ok(v) => match cirru_parser::format_expr_one_liner(&v) {
            Ok(s) => s,
            Err(_) => expr.lisp_str(),
          },
          Err(_) => expr.lisp_str(),
        }
      };
      let operator_desc = match cirru::calcit_to_cirru(a) {
        Ok(v) => match cirru_parser::format_expr_one_liner(&v) {
          Ok(s) => s,
          Err(_) => a.lisp_str(),
        },
        Err(_) => a.lisp_str(),
      };
      Err(CalcitErr::use_msg_stack_location_with_hint(
        CalcitErrKind::Type,
        format!("cannot be used as operator: {operator_desc} in {expr_one_liner}"),
        call_stack,
        location,
        "Possible: check if a leading `,` is needed to prevent a single-line call of Cirru syntax.",
      ))
    }
  }
}

pub fn evaluate_symbol(
  sym: &str,
  scope: &CalcitScope,
  file_ns: &str,
  at_def: &str,
  location: &Option<Arc<Vec<u16>>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let v = match parse_ns_def(sym) {
    Some((ns_part, def_part)) => match program::lookup_ns_target_in_import(file_ns, &ns_part) {
      Some(target_ns) => require_symbol_from_program(&def_part, &target_ns, call_stack),
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
      } else if builtins::is_registered_proc(sym) {
        Ok(Calcit::Registered(sym.into()))
      } else if let Some(v) = lookup_symbol_in_program_namespaces(sym, file_ns, call_stack)? {
        Ok(v)
      } else if let Some(target_ns) = program::lookup_def_target_in_import(file_ns, sym) {
        require_symbol_from_program(sym, &target_ns, call_stack)
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

pub fn evaluate_symbol_from_scope(idx: u16, sym: &str, scope: &CalcitScope) -> Result<Calcit, CalcitErr> {
  // Fast path: resolve by compiled local slot index.
  if let Some(v) = scope.get(idx) {
    return Ok(v.to_owned());
  }

  // Defensive fallback: resolve by symbol name so runtime does not panic when
  // local slot numbering drifts in edge macro/preprocess paths.
  if let Some(v) = scope.get_by_name(sym) {
    return Ok(v.to_owned());
  }

  let vars = scope.get_names();
  CalcitErr::err_str(CalcitErrKind::Var, format!("unknown local `{sym}`(#{idx}) in scope {vars}"))
}

/// a quick path of evaluating symbols, without checking scope and import
pub fn evaluate_symbol_from_program(
  sym: &str,
  file_ns: &str,
  def_id: Option<u32>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let v0 = resolve_runtime_or_compiled_def(file_ns, sym, def_id.map(program::DefId), call_stack)?;
  let v = if let Some(v) = v0 {
    v
  } else if let Some(v) = lookup_symbol_in_program_namespaces(sym, file_ns, call_stack)? {
    v
  } else {
    return Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Var,
      format!("expected symbol `{sym}` from path `{file_ns}`, this is a quick path, should succeed"),
      call_stack,
    ));
  };
  Ok(v)
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

/// resolve a program symbol to an available value for namespace lookup paths
pub fn eval_symbol_from_program(sym: &str, ns: &str, call_stack: &CallStackList) -> Result<Option<Calcit>, CalcitErr> {
  if let Some(v) = resolve_runtime_or_compiled_def(ns, sym, None, call_stack)? {
    return Ok(Some(v));
  }
  if program::has_def_code(ns, sym) {
    let warnings: RefCell<Vec<_>> = RefCell::new(vec![]);
    preprocess::ensure_ns_def_compiled(ns, sym, &warnings, call_stack)?;
    return resolve_runtime_or_compiled_def(ns, sym, None, call_stack);
  }
  Ok(None)
}

fn option_none_value(expected_type: &crate::calcit::CalcitTypeAnnotation) -> Option<Calcit> {
  if !expected_type.is_option_type() {
    return None;
  }
  let enum_def = expected_type.resolve_to_enum()?;
  let none_variant = enum_def.find_variant_by_name("none")?;
  if enum_def.name().ref_str() != "Option" || none_variant.arity() != 0 {
    return None;
  }
  Some(Calcit::Enum(CalcitEnumValue {
    tag: Arc::new(Calcit::Tag(EdnTag::new("none"))),
    extra: vec![],
    sum_type: Some(Arc::new(enum_def)),
  }))
}

/// Fill only a continuous trailing run of omitted `Option<T>` parameters.
/// A rest parameter keeps the function variadic and therefore disables this sugar.
fn complete_trailing_option_args(values: &[Calcit], info: &CalcitFn) -> Option<Vec<Calcit>> {
  if info.rest_type.is_some()
    || matches!(info.args.as_ref(), CalcitFnArgs::MarkedArgs(args) if args.iter().any(|arg| matches!(arg, CalcitArgLabel::RestMark)))
  {
    return None;
  }

  let param_len = info.args.param_len();
  let optional_count = trailing_option_arg_count(&info.arg_types, param_len);
  let required_len = param_len.saturating_sub(optional_count);
  if optional_count == 0 || values.len() < required_len || values.len() >= param_len {
    return None;
  }

  let missing = info.arg_types[values.len()..]
    .iter()
    .map(|expected| option_none_value(expected.as_ref()))
    .collect::<Option<Vec<_>>>()?;
  let mut completed = Vec::with_capacity(param_len);
  completed.extend_from_slice(values);
  completed.extend(missing);
  Some(completed)
}

pub fn run_fn(values: &[Calcit], info: &CalcitFn, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let completed_values = complete_trailing_option_args(values, info);
  let values = completed_values.as_deref().unwrap_or(values);
  let mut body_scope = (*info.scope).to_owned();
  let frame_checkpoint = body_scope.frame_checkpoint();
  match &*info.args {
    CalcitFnArgs::Args(args) => {
      if args.len() != values.len() {
        return Err(build_fn_arity_mismatch_error(info, values, call_stack, "call"));
      }
      for (&arg, value) in args.iter().zip(values) {
        body_scope.insert_mut(arg, value.to_owned());
      }
    }
    CalcitFnArgs::MarkedArgs(args) => bind_marked_args(&mut body_scope, args, values, call_stack)?,
  }

  let v = evaluate_lines(info.body.as_slice(), &body_scope, &info.def_ns, call_stack)?;

  if let Calcit::Recur(xs) = v {
    let mut current_values = xs.to_vec();
    loop {
      body_scope.restore_frame(frame_checkpoint);
      match &*info.args {
        CalcitFnArgs::Args(args) => {
          if args.len() != current_values.len() {
            return Err(build_fn_arity_mismatch_error(info, &current_values, call_stack, "recur"));
          }
          for (&arg, value) in args.iter().zip(&current_values) {
            body_scope.insert_mut(arg, value.to_owned());
          }
        }
        CalcitFnArgs::MarkedArgs(args) => bind_marked_args(&mut body_scope, args, &current_values, call_stack)?,
      }
      let v = evaluate_lines(info.body.as_slice(), &body_scope, &info.def_ns, call_stack)?;
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
  let values = complete_trailing_option_args(&values, info).unwrap_or(values);
  let mut body_scope = (*info.scope).to_owned();
  let frame_checkpoint = body_scope.frame_checkpoint();
  match &*info.args {
    CalcitFnArgs::Args(args) => {
      if args.len() != values.len() {
        return Err(build_fn_arity_mismatch_error(info, &values, call_stack, "call"));
      }
      for (&arg, value) in args.iter().zip(values) {
        body_scope.insert_mut(arg, value);
      }
    }
    CalcitFnArgs::MarkedArgs(args) => bind_marked_args(&mut body_scope, args, &values, call_stack)?,
  }

  let v = evaluate_lines(&info.body, &body_scope, &info.def_ns, call_stack)?;

  if let Calcit::Recur(xs) = v {
    let mut current_values = xs.to_vec();
    loop {
      body_scope.restore_frame(frame_checkpoint);
      match &*info.args {
        CalcitFnArgs::Args(args) => {
          if args.len() != current_values.len() {
            return Err(build_fn_arity_mismatch_error(info, &current_values, call_stack, "recur"));
          }
          for (&arg, value) in args.iter().zip(current_values) {
            body_scope.insert_mut(arg, value);
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
          let chunk = values[pop_values_idx.0..].to_vec();
          pop_values_idx.0 = values.len();
          scope.insert_mut(*idx, Calcit::from(CalcitList::Vector(chunk)));
          if pop_args_idx.0 < args.len() {
            return Err(CalcitErr::use_msg_stack(
              CalcitErrKind::Arity,
              format!("invalid argument declaration after `&` in signature `{}`", render_marked_args(args)),
              call_stack,
            ));
          }
        }
        _ => {
          return Err(CalcitErr::use_msg_stack(
            CalcitErrKind::Arity,
            format!("invalid argument declaration after `&` in signature `{}`", render_marked_args(args)),
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
                format!(
                  "too few values `{values:?}` for arguments `{}`; missing required argument `{}`",
                  render_marked_args(args),
                  CalcitLocal::read_name(*idx)
                ),
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
    let extra_count = values.len() - pop_values_idx.0;
    Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Arity,
      format!(
        "too many values `{values:?}` for arguments `{}`; {} extra value(s) are not handled",
        render_marked_args(args),
        extra_count
      ),
      call_stack,
    ))
  }
}

fn render_marked_args(args: &[CalcitArgLabel]) -> String {
  let mut parts: Vec<String> = vec![];
  for arg in args {
    match arg {
      CalcitArgLabel::RestMark => parts.push("&".to_owned()),
      CalcitArgLabel::OptionalMark => parts.push("?".to_owned()),
      CalcitArgLabel::Idx(idx) => parts.push(CalcitLocal::read_name(*idx).to_string()),
    }
  }
  format!("({})", parts.join(" "))
}

pub fn evaluate_lines(lines: &[Calcit], scope: &CalcitScope, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let mut ret: Calcit = Calcit::Nil;
  for line in lines {
    {
      let v = evaluate_expr(line, scope, file_ns, call_stack)?;
      ret = v
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
  evaluate_args_from(&items.view(), 0, scope, file_ns, call_stack)
}

pub fn evaluate_args_from(
  items: &CalcitListView<'_>,
  start: usize,
  scope: &CalcitScope,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<Vec<Calcit>, CalcitErr> {
  let mut ret: Vec<Calcit> = Vec::with_capacity(items.len().saturating_sub(start));
  let mut idx = 0;
  items.traverse_result::<CalcitErr>(&mut |item| {
    if idx < start {
      idx += 1;
      return Ok(());
    }
    idx += 1;

    if item.is_expr_evaluated() {
      ret.push(item.to_owned());
    } else {
      ret.push(evaluate_expr(item, scope, file_ns, call_stack)?);
    }
    Ok(())
  })?;
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
  evaluate_spreaded_args_from(&items.view(), 0, scope, file_ns, call_stack)
}

pub fn evaluate_spreaded_args_from(
  items: &CalcitListView<'_>,
  start: usize,
  scope: &CalcitScope,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<Vec<Calcit>, CalcitErr> {
  let mut ret: Vec<Calcit> = Vec::with_capacity(items.len().saturating_sub(start));
  let mut spreading = false;

  let mut idx = 0;
  items.traverse_result::<CalcitErr>(&mut |item| {
    if idx < start {
      idx += 1;
      return Ok(());
    }
    idx += 1;

    match item {
      Calcit::Syntax(CalcitSyntax::ArgSpread, _) => {
        spreading = true;
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
              }
              a => {
                return Err(CalcitErr::use_msg_stack_location(
                  CalcitErrKind::Arity,
                  format!("expected list for spreading, got: {a}"),
                  call_stack,
                  a.get_location(),
                ));
              }
            }
          } else {
            ret.push(item.to_owned());
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
              }
              a => {
                return Err(CalcitErr::use_msg_stack_location(
                  CalcitErrKind::Arity,
                  format!("expected list for spreading, got: {a}"),
                  call_stack,
                  a.get_location(),
                ));
              }
            }
          } else {
            ret.push(v);
          }
        }
      }
    }
    Ok(())
  })?;
  // println!("Evaluated args: {}", ret);
  Ok(ret)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{
    CalcitFnUsageMeta, CalcitMacro, CalcitStructDef, CalcitStructValue, CalcitSymbolInfo, CalcitTypeAnnotation, MacroCapability,
    MacroExpansionType, MacroSignature, MacroSignatureCompatibility,
  };
  use std::collections::HashSet;

  fn strict_test_macro(name: &str, body: Vec<Calcit>, capabilities: HashSet<MacroCapability>) -> Calcit {
    Calcit::Macro {
      id: Arc::from(format!("tests.runner/{name}")),
      info: Arc::new(CalcitMacro {
        name: Arc::from(name),
        def_ns: Arc::from("tests.runner"),
        args: Arc::new(vec![]),
        body: Arc::new(body),
        signature: Arc::new(MacroSignature {
          generics: Arc::new(vec![]),
          where_bounds: Arc::new(vec![]),
          required_inputs: Arc::new(vec![]),
          optional_inputs: Arc::new(vec![]),
          rest_input: None,
          expansion: MacroExpansionType::Expr(Arc::new(CalcitTypeAnnotation::String)),
          capabilities: Arc::new(capabilities),
          features: Arc::new(HashSet::new()),
          compatibility: MacroSignatureCompatibility::Strict,
        }),
      }),
    }
  }

  fn call_zero_arg_macro(value: &Calcit) -> Result<Calcit, CalcitErr> {
    let call = CalcitList::from(std::slice::from_ref(value));
    call_expr(
      value,
      &call.view(),
      &CalcitScope::default(),
      "tests.runner",
      &CallStackList::default(),
      false,
    )
  }

  #[test]
  fn strict_macro_runtime_path_enforces_declared_env_reads() {
    let body = vec![Calcit::from(vec![
      Calcit::Proc(CalcitProc::GetEnv),
      Calcit::new_str("CALCIT_TEST_CAPABILITY_MISSING_ENV"),
      Calcit::new_str("fallback"),
    ])];
    let pure = strict_test_macro("read-env", body.clone(), HashSet::new());
    let error = call_zero_arg_macro(&pure).expect_err("undeclared compile-time env read must fail");
    assert_eq!(error.code(), Some("E_MACRO_CAPABILITY_MISSING"));

    let declared = strict_test_macro("read-env", body, HashSet::from([MacroCapability::EnvRead]));
    assert_eq!(
      call_zero_arg_macro(&declared).expect("declared env read"),
      Calcit::new_str("fallback")
    );
  }

  #[test]
  fn strict_macro_may_emit_runtime_env_read_without_capability() {
    let runtime_call = Calcit::from(vec![
      Calcit::Proc(CalcitProc::GetEnv),
      Calcit::new_str("CALCIT_TEST_CAPABILITY_MISSING_ENV"),
      Calcit::new_str("runtime-fallback"),
    ]);
    let body = vec![Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Quote, Arc::from("tests.runner")),
      runtime_call,
    ])];
    let emitting_macro = strict_test_macro("emit-env", body, HashSet::new());
    assert_eq!(
      call_zero_arg_macro(&emitting_macro).expect("emitting runtime effect stays pure"),
      Calcit::new_str("runtime-fallback")
    );
  }

  fn local_value(name: &str, idx: u16) -> Calcit {
    Calcit::Local(CalcitLocal {
      idx,
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.runner"),
        at_def: Arc::from("tail-loop"),
      }),
      location: None,
      type_info: crate::calcit::DYNAMIC_TYPE.clone(),
    })
  }

  fn user_option_type() -> Arc<CalcitTypeAnnotation> {
    let option_def = crate::calcit::CalcitEnumDef::from_struct(CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(
        EdnTag::new("Option"),
        vec![EdnTag::new("some"), EdnTag::new("none")],
      )),
      values: Arc::new(vec![Calcit::from(vec![Calcit::tag("dynamic")]), Calcit::Nil]),
    })
    .expect("valid Option enum");
    Arc::new(CalcitTypeAnnotation::Enum(
      Arc::new(option_def),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
    ))
  }

  fn fn_info(arg_types: Vec<Arc<CalcitTypeAnnotation>>) -> CalcitFn {
    let args = (0..arg_types.len()).map(|idx| idx as u16).collect();
    CalcitFn {
      name: Arc::from("with-options"),
      def_ns: Arc::from("tests.runner"),
      def_ref: None,
      usage: CalcitFnUsageMeta::default(),
      scope: Arc::new(CalcitScope::default()),
      args: Arc::new(CalcitFnArgs::Args(args)),
      body: vec![],
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      arg_types,
      rest_type: None,
    }
  }

  #[test]
  fn does_not_fill_user_defined_option_enums() {
    let option = user_option_type();
    let info = fn_info(vec![Arc::new(CalcitTypeAnnotation::Number), option.clone(), option]);
    assert!(complete_trailing_option_args(&[Calcit::Number(1.0)], &info).is_none());
  }

  #[test]
  fn evaluates_specialized_number_binary_calls() {
    let cases = [
      (CalcitNumberBinaryOp::Add, 8.0, 2.0, Calcit::Number(10.0)),
      (CalcitNumberBinaryOp::Subtract, 8.0, 2.0, Calcit::Number(6.0)),
      (CalcitNumberBinaryOp::Multiply, 8.0, 2.0, Calcit::Number(16.0)),
      (CalcitNumberBinaryOp::Divide, 8.0, 2.0, Calcit::Number(4.0)),
      (CalcitNumberBinaryOp::Remainder, 8.0, 3.0, Calcit::Number(2.0)),
      (CalcitNumberBinaryOp::LessThan, 2.0, 8.0, Calcit::Bool(true)),
      (CalcitNumberBinaryOp::GreaterThan, 8.0, 2.0, Calcit::Bool(true)),
    ];

    for (operation, left, right, expected) in cases {
      let expr = Calcit::from(CalcitList::executable(
        vec![
          Calcit::Proc(number_binary_proc(operation)),
          Calcit::Number(left),
          Calcit::Number(right),
        ],
        CalcitCallKind::NumberBinary(operation),
      ));
      assert_eq!(
        evaluate_expr(&expr, &CalcitScope::default(), "tests.runner", &CallStackList::default()).expect("specialized call"),
        expected
      );
    }
  }

  #[test]
  fn specialized_number_call_preserves_dynamic_error_fallback() {
    let operation = CalcitNumberBinaryOp::Add;
    let expr = Calcit::from(CalcitList::executable(
      vec![
        Calcit::Proc(CalcitProc::NativeAdd),
        Calcit::Str(Arc::from("oops")),
        Calcit::Number(1.0),
      ],
      CalcitCallKind::NumberBinary(operation),
    ));
    let err = evaluate_expr(&expr, &CalcitScope::default(), "tests.runner", &CallStackList::default())
      .expect_err("stale static evidence must use the normal type error");

    assert!(format!("{err}").contains("&+ requires 2 numbers"));
  }

  #[test]
  fn specialized_remainder_matches_normal_number_conversion() {
    for (left, right) in [(8.0, 3.0), (8.5, 3.0), (f64::from(i32::MAX) + 1.0, 3.0), (8.0, 2.5)] {
      let expr = Calcit::from(CalcitList::executable(
        vec![
          Calcit::Proc(CalcitProc::NativeNumberRem),
          Calcit::Number(left),
          Calcit::Number(right),
        ],
        CalcitCallKind::NumberBinary(CalcitNumberBinaryOp::Remainder),
      ));
      let specialized = evaluate_expr(&expr, &CalcitScope::default(), "tests.runner", &CallStackList::default());
      let dispatched = builtins::handle_proc(
        CalcitProc::NativeNumberRem,
        &[Calcit::Number(left), Calcit::Number(right)],
        &CallStackList::default(),
      );

      assert_eq!(specialized, dispatched);
    }
  }

  #[test]
  fn specialized_remainder_preserves_normal_error_stack() {
    let expr = Calcit::from(CalcitList::executable(
      vec![Calcit::Proc(CalcitProc::NativeNumberRem), Calcit::Number(8.5), Calcit::Number(3.0)],
      CalcitCallKind::NumberBinary(CalcitNumberBinaryOp::Remainder),
    ));
    let call_stack = CallStackList::default().extend("tests.runner", "typed-rem-error", StackKind::Fn, &Calcit::Nil, &[]);
    let specialized =
      evaluate_expr(&expr, &CalcitScope::default(), "tests.runner", &call_stack).expect_err("fractional remainder input must fail");
    let dispatched = builtins::handle_proc(
      CalcitProc::NativeNumberRem,
      &[Calcit::Number(8.5), Calcit::Number(3.0)],
      &call_stack,
    )
    .expect_err("normal remainder dispatch must fail");

    assert_eq!(specialized.stack, dispatched.stack);
    assert_eq!(specialized.stack, call_stack);
  }

  #[test]
  fn mismatched_number_binary_metadata_uses_normal_dispatch() {
    let expr = Calcit::from(CalcitList::executable(
      vec![Calcit::Proc(CalcitProc::NativeMultiply), Calcit::Number(2.0), Calcit::Number(3.0)],
      CalcitCallKind::NumberBinary(CalcitNumberBinaryOp::Add),
    ));

    assert_eq!(
      evaluate_expr(&expr, &CalcitScope::default(), "tests.runner", &CallStackList::default())
        .expect("mismatched call metadata must use the stored procedure"),
      Calcit::Number(6.0)
    );
  }

  #[test]
  fn specialized_number_call_evaluates_arguments_left_to_right() {
    let failing_left = Calcit::from(vec![
      Calcit::Proc(CalcitProc::NativeAdd),
      Calcit::Str(Arc::from("left")),
      Calcit::Number(1.0),
    ]);
    let failing_right = Calcit::from(vec![
      Calcit::Proc(CalcitProc::NativeMinus),
      Calcit::Str(Arc::from("right")),
      Calcit::Number(1.0),
    ]);
    let expr = Calcit::from(CalcitList::executable(
      vec![Calcit::Proc(CalcitProc::NativeAdd), failing_left, failing_right],
      CalcitCallKind::NumberBinary(CalcitNumberBinaryOp::Add),
    ));
    let err = evaluate_expr(&expr, &CalcitScope::default(), "tests.runner", &CallStackList::default())
      .expect_err("left argument should fail first");

    assert!(format!("{err}").contains("&+ requires 2 numbers"));
  }

  #[test]
  fn long_recur_preserves_captured_scope() {
    let x = CalcitLocal::track_sym(&Arc::from("tail-loop-x"));
    let acc = CalcitLocal::track_sym(&Arc::from("tail-loop-acc"));
    let captured = CalcitLocal::track_sym(&Arc::from("tail-loop-captured"));
    let x_value = || local_value("tail-loop-x", x);
    let acc_value = || local_value("tail-loop-acc", acc);
    let captured_value = || local_value("tail-loop-captured", captured);

    let condition = Calcit::from(vec![Calcit::Proc(CalcitProc::NativeLessThan), x_value(), Calcit::Number(1.0)]);
    let completed = Calcit::from(vec![Calcit::Proc(CalcitProc::NativeAdd), acc_value(), captured_value()]);
    let next_x = Calcit::from(vec![Calcit::Proc(CalcitProc::NativeMinus), x_value(), Calcit::Number(1.0)]);
    let next_acc = Calcit::from(vec![Calcit::Proc(CalcitProc::NativeAdd), acc_value(), Calcit::Number(1.0)]);
    let recur = Calcit::from(vec![Calcit::Proc(CalcitProc::Recur), next_x, next_acc]);
    let body = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::If, Arc::from("tests.runner")),
      condition,
      completed,
      recur,
    ]);

    let mut closure_scope = CalcitScope::default();
    closure_scope.insert_mut(captured, Calcit::Number(42.0));
    let info = CalcitFn {
      name: Arc::from("tail-loop"),
      def_ns: Arc::from("tests.runner"),
      def_ref: None,
      usage: CalcitFnUsageMeta::default(),
      scope: Arc::new(closure_scope),
      args: Arc::new(CalcitFnArgs::Args(vec![x, acc])),
      body: vec![body],
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      return_type: Arc::new(CalcitTypeAnnotation::Number),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::Number)],
      rest_type: None,
    };

    let result = run_fn_owned(
      vec![Calcit::Number(100_000.0), Calcit::Number(0.0)],
      &info,
      &CallStackList::default(),
    )
    .expect("long recur should complete");

    assert_eq!(result, Calcit::Number(100_042.0));
  }
}
