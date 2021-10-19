use std::cmp::Ordering;
use std::cmp::Ordering::*;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum CalcitSyntax {
  Defn,
  Defmacro,
  If,
  CoreLet,
  Quote,
  Quasiquote,
  Eval,
  Macroexpand,
  Macroexpand1,
  MacroexpandAll,
  Try,
  Defatom,
  Reset,
  HintFn,
}

use CalcitSyntax::*;

impl fmt::Display for CalcitSyntax {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let buf = match self {
      Defn => "defn",
      Defmacro => "defmacro",
      If => "if",
      CoreLet => "&let",
      Quote => "quote",
      Quasiquote => "quasiquote",
      Eval => "eval",
      Macroexpand => "macroexpand",
      Macroexpand1 => "macroexpand-1",
      MacroexpandAll => "macroexpand-all",
      Try => "try",
      Defatom => "defatom",
      Reset => "reset!",
      HintFn => "hint-fn",
    };
    f.write_str(buf) // TODO performance
  }
}

impl CalcitSyntax {
  pub fn from(s: &str) -> Result<Self, String> {
    match s {
      "defn" => Ok(Defn),
      "defmacro" => Ok(Defmacro),
      "if" => Ok(If),
      "&let" => Ok(CoreLet),
      "quote" => Ok(Quote),
      "quasiquote" => Ok(Quasiquote),
      "eval" => Ok(Eval),
      "macroexpand" => Ok(Macroexpand),
      "macroexpand-1" => Ok(Macroexpand1),
      "macroexpand-all" => Ok(MacroexpandAll),
      "try" => Ok(Try),
      "defatom" => Ok(Defatom),
      "reset!" => Ok(Reset),
      "hint-fn" => Ok(HintFn),
      _ => Err(format!("Unknown format! {}", s)),
    }
  }

  pub fn is_core_syntax(s: &str) -> bool {
    matches!(
      s,
      "defn"
        | "defmacro"
        | "if"
        | "&let"
        | "quote"
        | "quasiquote"
        | "eval"
        | "macroexpand"
        | "macroexpand-1"
        | "macroexpand-all"
        | "try"
        | "defatom"
        | "reset!"
        | "hint-fn"
    )
  }
}

impl Ord for CalcitSyntax {
  fn cmp(&self, other: &Self) -> Ordering {
    match (self, other) {
      (Defn, Defn) => Equal,
      (Defn, _) => Less,
      (_, Defn) => Greater,

      (Defmacro, Defmacro) => Equal,
      (Defmacro, _) => Less,
      (_, Defmacro) => Greater,

      (If, If) => Equal,
      (If, _) => Less,
      (_, If) => Greater,

      (CoreLet, CoreLet) => Equal,
      (CoreLet, _) => Less,
      (_, CoreLet) => Greater,

      (Quote, Quote) => Equal,
      (Quote, _) => Less,
      (_, Quote) => Greater,

      (Quasiquote, Quasiquote) => Equal,
      (Quasiquote, _) => Less,
      (_, Quasiquote) => Greater,

      (Eval, Eval) => Equal,
      (Eval, _) => Less,
      (_, Eval) => Greater,

      (Macroexpand, Macroexpand) => Equal,
      (Macroexpand, _) => Less,
      (_, Macroexpand) => Greater,

      (Macroexpand1, Macroexpand1) => Equal,
      (Macroexpand1, _) => Less,
      (_, Macroexpand1) => Greater,

      (MacroexpandAll, MacroexpandAll) => Equal,
      (MacroexpandAll, _) => Less,
      (_, MacroexpandAll) => Greater,

      (Try, Try) => Equal,
      (Try, _) => Less,
      (_, Try) => Greater,

      (Defatom, Defatom) => Equal,
      (Defatom, _) => Less,
      (_, Defatom) => Greater,

      (Reset, Reset) => Equal,
      (Reset, _) => Less,
      (_, Reset) => Greater,

      (HintFn, HintFn) => Equal,
    }
  }
}

impl Eq for CalcitSyntax {}

impl PartialOrd for CalcitSyntax {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}
