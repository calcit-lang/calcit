//! Tail-position analysis for `recur`.
//!
//! A `recur` call produces a `Recur` value that only the enclosing function (or macro)
//! body consumes, by restarting itself. The value must therefore flow straight to the
//! body's return: anywhere else it would leak as plain data or be silently dropped.
//! The same analysis serves preprocessing (warnings) and the JS/WASM backends (codegen
//! errors), so every backend applies one rule to the processed code.

use crate::calcit::{Calcit, CalcitList, CalcitProc, CalcitSyntax};

/// Returns the forms of a processed function body that use `recur` outside tail position.
///
/// Tail position flows through `if` branches, the last `&let` body form, the `try` body
/// and `match` branch bodies. `recur` must be the callee of a direct call or of
/// `&call-spread` there; passing `recur` itself around as a value is also reported,
/// since its result could not be traced back to the function body.
/// Nested functions and quoted code are skipped, they own their `recur`.
pub fn non_tail_recur_forms(body: &[Calcit]) -> Vec<&Calcit> {
  // the last non-hint form is the returned value, earlier forms are evaluated for effects only
  let tail_index = body
    .iter()
    .rposition(|form| !matches!(form, Calcit::List(xs) if matches!(xs.first(), Some(Calcit::Syntax(CalcitSyntax::HintFn, _)))));
  let mut found = vec![];
  for (idx, form) in body.iter().enumerate() {
    walk(form, form, Some(idx) == tail_index, &mut found);
  }
  found
}

fn walk<'a>(expr: &'a Calcit, parent: &'a Calcit, in_tail: bool, found: &mut Vec<&'a Calcit>) {
  match expr {
    // `recur` referenced as a value instead of being called
    Calcit::Proc(CalcitProc::Recur) => found.push(parent),
    Calcit::Recur(args) => {
      if !in_tail {
        found.push(expr);
      }
      for arg in args {
        walk(arg, expr, false, found);
      }
    }
    Calcit::List(xs) => {
      let Some(head) = xs.first() else {
        return;
      };
      match head {
        Calcit::Syntax(CalcitSyntax::If, _) => {
          for (idx, item) in xs.iter().enumerate().skip(1) {
            walk(item, expr, in_tail && idx >= 2, found);
          }
        }
        Calcit::Syntax(CalcitSyntax::CoreLet, _) => {
          let last = xs.len() - 1;
          for (idx, item) in xs.iter().enumerate().skip(1) {
            if idx == 1 {
              // binding pair: the bound value is never in tail position
              if let Calcit::List(pair) = item {
                for value in pair.iter().skip(1) {
                  walk(value, expr, false, found);
                }
              }
            } else {
              walk(item, expr, in_tail && idx == last, found);
            }
          }
        }
        // `(try body handler)`: the body value is returned as is, the handler is a separate function
        Calcit::Syntax(CalcitSyntax::Try, _) => {
          for (idx, item) in xs.iter().enumerate().skip(1) {
            walk(item, expr, in_tail && idx == 1, found);
          }
        }
        Calcit::Syntax(CalcitSyntax::Match, _) => {
          if let Some(value) = xs.get(1) {
            walk(value, expr, false, found);
          }
          // a statically known enum match is preprocessed into `(match value enum-def branch-table)`
          if xs.len() == 4
            && matches!(xs.get(2), Some(Calcit::EnumDef(..)))
            && let Some(Calcit::List(table)) = xs.get(3)
          {
            for slot in table.iter() {
              walk_match_branch(slot, in_tail, found);
            }
          } else {
            for branch in xs.iter().skip(2) {
              walk_match_branch(branch, in_tail, found);
            }
          }
        }
        // `(&call-spread f args...)` calls `f` like a plain call
        Calcit::Syntax(CalcitSyntax::CallSpread, _) => walk_call(expr, xs, 1, in_tail, found),
        // nested functions and macros own their `recur`, quoted code is data
        Calcit::Syntax(
          CalcitSyntax::Defn
          | CalcitSyntax::DefWasmExport
          | CalcitSyntax::DefWasmImport
          | CalcitSyntax::Defmacro
          | CalcitSyntax::Quote
          | CalcitSyntax::Quasiquote
          | CalcitSyntax::Gensym
          | CalcitSyntax::HintFn
          | CalcitSyntax::Macroexpand
          | CalcitSyntax::Macroexpand1
          | CalcitSyntax::MacroexpandAll
          | CalcitSyntax::MacroInterpolate
          | CalcitSyntax::MacroInterpolateSpread,
          _,
        ) => {}
        // other syntax forms consume their operands, none of which is in tail position
        Calcit::Syntax(_, _) => {
          for item in xs.iter().skip(1) {
            walk(item, expr, false, found);
          }
        }
        _ => walk_call(expr, xs, 0, in_tail, found),
      }
    }
    _ => {}
  }
}

/// A call whose callee sits at `start`; arguments are never in tail position.
fn walk_call<'a>(expr: &'a Calcit, xs: &'a CalcitList, start: usize, in_tail: bool, found: &mut Vec<&'a Calcit>) {
  match xs.get(start) {
    Some(Calcit::Proc(CalcitProc::Recur)) => {
      if !in_tail {
        found.push(expr);
      }
    }
    Some(callee) => walk(callee, expr, false, found),
    None => {}
  }
  for item in xs.iter().skip(start + 1) {
    walk(item, expr, false, found);
  }
}

fn walk_match_branch<'a>(branch: &'a Calcit, in_tail: bool, found: &mut Vec<&'a Calcit>) {
  // a branch is `(pattern body)`, patterns only bind names
  if let Calcit::List(pair) = branch
    && let Some(body) = pair.get(1)
  {
    walk(body, branch, in_tail, found);
  }
}
