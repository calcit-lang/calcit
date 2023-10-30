use std::cmp::Ordering;
use std::cmp::Ordering::*;

/// core syntax inside Calcit
#[derive(Debug, Clone, PartialEq, EnumString, strum_macros::Display)]
pub enum CalcitSyntax {
  #[strum(serialize = "defn")]
  Defn,
  #[strum(serialize = "defmacro")]
  Defmacro,
  #[strum(serialize = "if")]
  If,
  /// `&let` that binds only 1 local
  #[strum(serialize = "&let")]
  CoreLet,
  /// to turn code into quoted data
  #[strum(serialize = "quote")]
  Quote,
  /// used inside macro
  #[strum(serialize = "quasiquote")]
  Quasiquote,
  #[strum(serialize = "gensym")]
  Gensym,
  #[strum(serialize = "eval")]
  Eval,
  /// expand macro until recursive calls are resolved
  #[strum(serialize = "macroexpand")]
  Macroexpand,
  /// expand macro just once for debugging, even `Recur` is returned
  #[strum(serialize = "macroexpand-1")]
  Macroexpand1,
  /// expand macro until macros inside are resolved
  #[strum(serialize = "macroexpand-all")]
  MacroexpandAll,
  /// it has special behaviors of try catch
  #[strum(serialize = "try")]
  Try,
  /// referenced state defined and attached undefined namespace
  #[strum(serialize = "defatom")]
  Defatom,
  /// `reset!` value to atom
  #[strum(serialize = "reset!")]
  Reset,
  /// a hint mark inside function, currently only used for `async`
  #[strum(serialize = "hint-fn")]
  HintFn,
}

use strum_macros::EnumString;
use CalcitSyntax::*;

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
