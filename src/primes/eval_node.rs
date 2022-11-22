//! TODO
//! "Eval Node" as an intermediate representation of AST.
//! previous implementation was using Calcit List and less optimizations can be applied.

use std::sync::Arc;

use crate::primes::Calcit;
use crate::primes::CalcitProc;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum EvalNode {
  Proc(CalcitProc, Vec<EvalNode>),
  CallFn(Vec<EvalNode>),
  CallFnSpread(Vec<EvalNode>),
  Data(Calcit),
  /// TODO
  Defn(String),
  If(Arc<EvalNode>, Arc<EvalNode>, Arc<EvalNode>),
  /// TODO
  Defmacro(String),
  CoreLet(String, Arc<EvalNode>, Vec<EvalNode>),
  Quote(Arc<EvalNode>),
  Quasiquote(Arc<EvalNode>),
  Gensym(String),
  Eval(Arc<EvalNode>),
  Macroexpand(Arc<EvalNode>),
  Macroexpand1(Arc<EvalNode>),
  MacroexpandAll(Arc<EvalNode>),
  Try(Arc<EvalNode>, Arc<EvalNode>),
  Defatom(String, Arc<EvalNode>),
  Reset(String, Arc<EvalNode>),
}
