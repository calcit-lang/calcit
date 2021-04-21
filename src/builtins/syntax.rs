use crate::builtins;
use crate::primes;
use crate::primes::{Calcit, CalcitItems, CalcitScope};
use crate::program::ProgramCodeData;
use crate::runner;

pub fn defn(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  _program: &ProgramCodeData,
) -> Result<Calcit, String> {
  match (expr.get(0), expr.get(1)) {
    (Some(Calcit::Symbol(s, ..)), Some(Calcit::List(xs))) => Ok(Calcit::Fn(
      s.to_string(),
      file_ns.to_string(),
      nanoid!(),
      scope.clone(),
      xs.clone(),
      expr.clone().slice(2..),
    )),
    (Some(a), Some(b)) => Err(format!("invalid args type for defn: {} , {}", a, b)),
    _ => Err(String::from("inefficient arguments for defn")),
  }
}

pub fn defmacro(
  expr: &CalcitItems,
  _scope: &CalcitScope,
  def_ns: &str,
  _program: &ProgramCodeData,
) -> Result<Calcit, String> {
  match (expr.get(0), expr.get(1)) {
    (Some(Calcit::Symbol(s, ..)), Some(Calcit::List(xs))) => Ok(Calcit::Macro(
      s.to_string(),
      def_ns.to_string(),
      nanoid!(),
      xs.clone(),
      expr.clone().slice(2..),
    )),
    (Some(a), Some(b)) => Err(format!("invalid structure for defmacro: {} {}", a, b)),
    _ => Err(format!(
      "invalid structure for defmacro: {}",
      Calcit::List(expr.clone())
    )),
  }
}

pub fn quote(
  expr: &CalcitItems,
  _scope: &CalcitScope,
  _file_ns: &str,
  _program: &ProgramCodeData,
) -> Result<Calcit, String> {
  if expr.len() == 1 {
    Ok(expr[0].clone())
  } else {
    Err(format!("unexpected data for quote: {:?}", expr))
  }
}

pub fn syntax_if(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, String> {
  match (expr.get(0), expr.get(1)) {
    _ if expr.len() > 3 => Err(format!("too many nodes for if: {:?}", expr)),
    (Some(cond), Some(true_branch)) => {
      let cond_value = runner::evaluate_expr(cond, scope, file_ns, program_code)?;
      match cond_value {
        Calcit::Nil | Calcit::Bool(false) => match expr.get(2) {
          Some(false_branch) => runner::evaluate_expr(false_branch, scope, file_ns, program_code),
          None => Ok(Calcit::Nil),
        },
        _ => runner::evaluate_expr(true_branch, scope, file_ns, program_code),
      }
    }
    (None, _) => Err(format!("insufficient nodes for if: {:?}", expr)),
    _ => Err(format!("invalid if form: {:?}", expr)),
  }
}

pub fn eval(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, String> {
  if expr.len() == 1 {
    let v = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;
    runner::evaluate_expr(&v, scope, file_ns, program_code)
  } else {
    Err(format!("unexpected data for evaling: {:?}", expr))
  }
}

pub fn syntax_let(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, String> {
  match expr.get(0) {
    Some(Calcit::Nil) => {
      runner::evaluate_lines(&expr.clone().slice(1..), scope, file_ns, program_code)
    }
    Some(Calcit::List(xs)) if xs.len() == 2 => {
      let mut body_scope = scope.clone();
      match (&xs[0], &xs[1]) {
        (Calcit::Symbol(s, ..), ys) => {
          let value = runner::evaluate_expr(ys, scope, file_ns, program_code)?;
          body_scope.insert(s.to_string(), value);
        }
        (a, _) => return Err(format!("invalid binding name: {}", a)),
      }
      runner::evaluate_lines(&expr.clone().slice(1..), &body_scope, file_ns, program_code)
    }
    Some(Calcit::List(xs)) => Err(format!("invalid length: {:?}", xs)),
    Some(_) => Err(format!("invalid node for &let: {:?}", expr)),
    None => Err(String::from("&let expected a pair or a nil")),
  }
}

/// foldl using syntax for performance, theoretically it's not
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
          ret = runner::run_fn(values, &def_scope, args, body, def_ns, program_code)?;
        }
        Ok(ret)
      }
      (Calcit::List(xs), Calcit::Proc(proc)) => {
        let mut ret = acc;
        for x in xs {
          ret = builtins::handle_proc(&proc, &im::vector![ret, x.clone()])?;
        }
        Ok(ret)
      }

      (_, _) => Err(format!(
        "foldl expected list and function, got: {} {}",
        xs, f
      )),
    }
  } else {
    Err(format!("foldl expected 3 arguments, got: {:?}", expr))
  }
}

pub fn quasiquote(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, String> {
  match expr.get(0) {
    None => Err(String::from("quasiquote expected a node")),
    Some(code) => {
      let ret = replace_code(code, scope, file_ns, program_code)?;
      match ret.get(0) {
        Some(v) => {
          // println!("replace result: {:?}", v);
          Ok(v.clone())
        }
        None => Err(String::from("missing quote expr")),
      }
    }
  }
}

fn replace_code(
  c: &Calcit,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<CalcitItems, String> {
  match c {
    Calcit::List(ys) => match (ys.get(0), ys.get(1)) {
      (Some(Calcit::Symbol(sym, ..)), Some(expr)) if sym == "~" => {
        let value = runner::evaluate_expr(expr, scope, file_ns, program_code)?;
        Ok(im::vector![value])
      }
      (Some(Calcit::Symbol(sym, ..)), Some(expr)) if sym == "~@" => {
        let ret = runner::evaluate_expr(expr, scope, file_ns, program_code)?;
        match ret {
          Calcit::List(zs) => Ok(zs),
          _ => Err(format!("unknown result from unquite-slice: {}", ret)),
        }
      }
      (_, _) => {
        let mut ret = im::vector![];
        for y in ys {
          let pieces = replace_code(y, scope, file_ns, program_code)?;
          for piece in pieces {
            ret.push_back(piece);
          }
        }
        Ok(im::vector![Calcit::List(ret)])
      }
    },
    _ => Ok(im::vector![c.clone()]),
  }
}

pub fn macroexpand(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, String> {
  if expr.len() == 1 {
    let quoted_code = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;

    match quoted_code.clone() {
      Calcit::List(xs) => {
        if xs.is_empty() {
          return Ok(quoted_code);
        }
        let v = runner::evaluate_expr(&xs[0], scope, file_ns, program_code)?;
        match v {
          Calcit::Macro(_, def_ns, _, args, body) => {
            let mut xs_cloned = xs;
            // mutable operation
            let mut rest_nodes = xs_cloned.slice(1..);
            // println!("macro: {:?} ... {:?}", args, rest_nodes);
            // keep expanding until return value is not a recur
            loop {
              let body_scope = runner::bind_args(&args, &rest_nodes, scope)?;
              let v = runner::evaluate_lines(&body, &body_scope, &def_ns, program_code)?;
              match v {
                Calcit::Recur(rest_code) => {
                  rest_nodes = rest_code;
                }
                _ => return Ok(v),
              }
            }
          }
          _ => Ok(quoted_code),
        }
      }
      a => Ok(a),
    }
  } else {
    Err(format!(
      "macroexpand expected excaclty 1 argument, got: {:?}",
      expr
    ))
  }
}

pub fn macroexpand_1(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, String> {
  if expr.len() == 1 {
    let quoted_code = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;
    // println!("quoted: {}", quoted_code);
    match quoted_code.clone() {
      Calcit::List(xs) => {
        if xs.is_empty() {
          return Ok(quoted_code);
        }
        let v = runner::evaluate_expr(&xs[0], scope, file_ns, program_code)?;
        match v {
          Calcit::Macro(_, def_ns, _, args, body) => {
            let mut xs_cloned = xs;
            let body_scope = runner::bind_args(&args, &xs_cloned.slice(1..), scope)?;
            runner::evaluate_lines(&body, &body_scope, &def_ns, program_code)
          }
          _ => Ok(quoted_code),
        }
      }
      a => Ok(a),
    }
  } else {
    Err(format!(
      "macroexpand expected excaclty 1 argument, got: {:?}",
      expr
    ))
  }
}

pub fn call_try(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, String> {
  if expr.len() == 2 {
    let xs = runner::evaluate_expr(&expr[0], scope, file_ns, program_code);

    match &xs {
      // dirty since only functions being call directly then we become fast
      Ok(v) => Ok(v.clone()),
      Err(failure) => {
        let f = runner::evaluate_expr(&expr[1], scope, file_ns, program_code)?;
        let err_data = Calcit::Str(failure.to_string());
        match f {
          Calcit::Fn(_, def_ns, _, def_scope, args, body) => {
            let values = im::vector![err_data];
            runner::run_fn(values, &def_scope, &args, &body, &def_ns, program_code)
          }
          Calcit::Proc(proc) => builtins::handle_proc(&proc, &im::vector![err_data]),
          a => Err(format!("try expected a function handler, got: {}", a)),
        }
      }
    }
  } else {
    Err(format!("try expected 2 arguments, got: {:?}", expr))
  }
}
