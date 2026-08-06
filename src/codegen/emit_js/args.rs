use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt::Write;
use std::sync::Arc;

use cirru_edn::EdnTag;

use crate::builtins::meta::js_gensym;
use crate::calcit::{self, Calcit, CalcitList, CalcitProc, CalcitSyntax};

use super::symbols::escape_var;
use super::{ImportsDict, to_js_code};

pub(super) fn gen_args_code(
  body: &CalcitList,
  ns: &str,
  local_defs: &HashSet<Arc<str>>,
  file_imports: &RefCell<ImportsDict>,
  tags: &RefCell<HashSet<EdnTag>>,
) -> Result<String, String> {
  let mut result = String::from("");
  let var_prefix = if ns == "calcit.core" { "" } else { "$clt." };
  let mut spreading = false;
  for x in body {
    match x {
      Calcit::Syntax(CalcitSyntax::ArgSpread, _) => {
        spreading = true;
      }
      _ => {
        if !result.is_empty() {
          result.push_str(", ")
        }
        if spreading {
          result.push_str("...");
          result.push_str(var_prefix);
          result.push_str("listToArray(");
          result.push_str(&to_js_code(x, ns, local_defs, file_imports, tags, None)?);
          result.push(')');
          spreading = false
        } else {
          result.push_str(&to_js_code(x, ns, local_defs, file_imports, tags, None)?);
        }
      }
    }
  }
  Ok(result)
}

pub(super) fn gen_call_args_with_temps(
  body: &CalcitList,
  ns: &str,
  local_defs: &HashSet<Arc<str>>,
  file_imports: &RefCell<ImportsDict>,
  tags: &RefCell<HashSet<EdnTag>>,
  aggressive: bool,
  inline_all: bool,
) -> Result<(String, String), String> {
  let mut prelude = String::from("");
  let mut args_code = String::from("");
  let mut spreading = false;
  let var_prefix = if ns == calcit::CORE_NS { "" } else { "$clt." };
  let max_depth = if aggressive {
    0
  } else if body.len() <= 2 {
    2
  } else if body.len() > 4 {
    0
  } else {
    1
  };

  for x in body {
    match x {
      Calcit::Syntax(CalcitSyntax::ArgSpread, _) => {
        spreading = true;
      }
      _ => {
        if !args_code.is_empty() {
          args_code.push_str(", ");
        }

        let should_inline = if is_complex_syntax(x) {
          false
        } else if inline_all {
          true
        } else {
          is_expr_depth_at_most(x, max_depth)
        };

        let arg_code = if should_inline {
          to_js_code(x, ns, local_defs, file_imports, tags, None)?
        } else {
          let tmp = escape_var(&js_gensym("tmp"));
          let expr = to_js_code(x, ns, local_defs, file_imports, tags, None)?;
          writeln!(prelude, "let {tmp} = {expr};").expect("write");
          tmp
        };

        if spreading {
          write!(args_code, "...{var_prefix}listToArray({arg_code})").expect("write");
          spreading = false;
        } else {
          args_code.push_str(&arg_code);
        }
      }
    }
  }

  Ok((prelude, args_code))
}

fn is_simple_expr(x: &Calcit) -> bool {
  match x {
    Calcit::Local { .. }
    | Calcit::Symbol { .. }
    | Calcit::Number(_)
    | Calcit::Bool(_)
    | Calcit::Nil
    | Calcit::Str(_)
    | Calcit::Tag(_)
    | Calcit::Import(_)
    | Calcit::Fn { .. }
    | Calcit::Thunk(..)
    | Calcit::Proc(..)
    | Calcit::Method(..) => true,
    Calcit::Syntax(..) => true,
    Calcit::List(xs) => xs.is_empty(),
    _ => false,
  }
}

fn is_expr_depth_at_most(x: &Calcit, depth: usize) -> bool {
  if depth == 0 {
    return is_simple_expr(x);
  }
  match x {
    Calcit::List(xs) => {
      if xs.is_empty() {
        return true;
      }
      let next_depth = if xs.len() <= 4 && depth < 4 { depth + 1 } else { depth };
      xs.iter().all(|item| is_expr_depth_at_most(item, next_depth - 1))
    }
    _ => is_simple_expr(x),
  }
}

fn is_complex_syntax(x: &Calcit) -> bool {
  match x {
    Calcit::List(xs) => match xs.first() {
      Some(Calcit::Syntax(syn, _)) => matches!(
        syn,
        CalcitSyntax::If
          | CalcitSyntax::Try
          | CalcitSyntax::CoreLet
          | CalcitSyntax::Defn
          | CalcitSyntax::DefWasmExport
          | CalcitSyntax::DefWasmImport
          | CalcitSyntax::Defmacro
      ),
      Some(Calcit::Proc(CalcitProc::Raise)) => true,
      _ => false,
    },
    _ => false,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn joins_plain_args() {
    let body = CalcitList::from(&[Calcit::Number(1.0), Calcit::Bool(true)]);
    let local_defs: HashSet<Arc<str>> = HashSet::new();
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());

    let code = gen_args_code(&body, "app.main", &local_defs, &file_imports, &tags).expect("gen args");
    assert_eq!(code, "1, true");
  }

  #[test]
  fn wraps_spread_with_list_to_array() {
    let body = CalcitList::from(&[Calcit::Syntax(CalcitSyntax::ArgSpread, Arc::from("app.main")), Calcit::Number(1.0)]);
    let local_defs: HashSet<Arc<str>> = HashSet::new();
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());

    let code = gen_args_code(&body, "app.main", &local_defs, &file_imports, &tags).expect("gen args");
    assert_eq!(code, "...$clt.listToArray(1)");
  }
}
