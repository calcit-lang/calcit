use strum_macros::{AsRefStr, EnumString};

/// core syntax inside Calcit
#[derive(Debug, Clone, PartialEq, EnumString, strum_macros::Display, AsRefStr, PartialOrd, Eq, Ord)]
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
  /// special call for handling `&` spreading
  #[strum(serialize = "&call-spread")]
  CallSpread,
  /// spreading in function definition and call
  #[strum(serialize = "&")]
  ArgSpread,
  /// optional argument in function definition
  #[strum(serialize = "?")]
  ArgOptional,
  /// interpolate value in macro
  #[strum(serialize = "~")]
  MacroInterpolate,
  /// spreading interpolate value in macro
  #[strum(serialize = "~@")]
  MacroInterpolateSpread,
  /// placeholder for upcoming local type annotations
  #[strum(serialize = "asset-type")]
  AssetType,
}

impl CalcitSyntax {
  /// check is given name is a syntax name
  pub fn is_valid(s: &str) -> bool {
    s.parse::<CalcitSyntax>().is_ok()
  }
}
