/// core syntax inside Calcit
#[derive(Debug, Clone, PartialEq, EnumString, strum_macros::Display, PartialOrd, Eq, Ord)]
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

impl CalcitSyntax {
  /// check is given name is a syntax name
  pub fn is_valid(s: &str) -> bool {
    s.parse::<CalcitSyntax>().is_ok()
  }
}
