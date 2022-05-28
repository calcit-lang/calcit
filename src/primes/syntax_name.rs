use std::cmp::Ordering;
use std::cmp::Ordering::*;
use std::fmt;
use std::sync::Arc;

/// core syntax inside Calcit
#[derive(Debug, Clone, PartialEq)]
pub enum CalcitSyntax {
  Defn,
  Defmacro,
  If,
  /// `&let` that binds only 1 local
  CoreLet,
  /// to turn code into quoted data
  Quote,
  /// used inside macro
  Quasiquote,
  Gensym,
  Eval,
  /// expand macro until recursive calls are resolved
  Macroexpand,
  /// expand macro just once for debugging, even `Recur` is returned
  Macroexpand1,
  /// expand macro until macros inside are resolved
  MacroexpandAll,
  /// it has special behaviors of try catch
  Try,
  /// referenced state defined and attached undefined namespace
  Defatom,
  /// `reset!` value to atom
  Reset,
  /// a hint mark inside function, currently only used for `async`
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
      Gensym => "gensym",
      Eval => "eval",
      Macroexpand => "macroexpand",
      Macroexpand1 => "macroexpand-1",
      MacroexpandAll => "macroexpand-all",
      Try => "try",
      Defatom => "defatom",
      Reset => "reset!",
      HintFn => "hint-fn",
    };
    f.write_str(buf)
  }
}

impl TryFrom<&str> for CalcitSyntax {
  type Error = String;
  fn try_from(s: &str) -> Result<Self, Self::Error> {
    match s {
      "defn" => Ok(Defn),
      "defmacro" => Ok(Defmacro),
      "if" => Ok(If),
      "&let" => Ok(CoreLet),
      "quote" => Ok(Quote),
      "quasiquote" => Ok(Quasiquote),
      "gensym" => Ok(Gensym),
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
}

impl TryFrom<Arc<str>> for CalcitSyntax {
  type Error = String;
  fn try_from(s: Arc<str>) -> Result<Self, Self::Error> {
    Self::try_from(&*s)
  }
}

impl TryFrom<&Arc<str>> for CalcitSyntax {
  type Error = String;
  fn try_from(s: &Arc<str>) -> Result<Self, Self::Error> {
    Self::try_from(&**s)
  }
}

impl CalcitSyntax {
  /// check is given name is a syntax name
  pub fn is_valid(s: &str) -> bool {
    matches!(
      s,
      "defn"
        | "defmacro"
        | "if"
        | "&let"
        | "quote"
        | "quasiquote"
        | "gensym"
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

      (Gensym, Gensym) => Equal,
      (Gensym, _) => Less,
      (_, Gensym) => Greater,

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
