use std::sync::Arc;

use strum_macros::{AsRefStr, EnumString};

use crate::calcit::CalcitTypeAnnotation;

#[derive(Debug, Clone)]
pub struct SyntaxTypeSignature {
  pub param_names: Vec<&'static str>,
  pub param_types: Vec<Arc<CalcitTypeAnnotation>>,
  pub return_type: Arc<CalcitTypeAnnotation>,
}

/// core syntax inside Calcit
#[derive(Debug, Clone, PartialEq, EnumString, strum_macros::Display, AsRefStr, PartialOrd, Eq, Ord)]
pub enum CalcitSyntax {
  #[strum(serialize = "defn")]
  Defn,
  /// Define a function that is exported from a generated WASM module.
  #[strum(serialize = "defwasm-export")]
  DefWasmExport,
  /// Declare a host function imported by a generated WASM module.
  #[strum(serialize = "defwasm-import")]
  DefWasmImport,
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
  #[strum(serialize = "assert-type")]
  AssertType,
  /// Explicitly attach a type annotation without a runtime validation.
  #[strum(serialize = "unsafe-coerce")]
  UnsafeCoerce,
  /// Parse Cirru EDN and deeply validate/construct the declared closed type.
  #[strum(serialize = "parse-cirru-edn-as")]
  ParseCirruEdnAs,
  /// placeholder for trait requirement assertions
  #[strum(serialize = "assert-traits")]
  AssertTraits,
  /// pattern matching on enums with exhaustiveness detection
  #[strum(serialize = "match")]
  Match,
}

impl CalcitSyntax {
  /// check is given name is a syntax name
  pub fn is_valid(s: &str) -> bool {
    s.parse::<CalcitSyntax>().is_ok()
  }

  pub fn get_type_signature(&self) -> Option<SyntaxTypeSignature> {
    use CalcitSyntax::*;
    let dyn_t = Arc::new(CalcitTypeAnnotation::Dynamic);
    let bool_t = Arc::new(CalcitTypeAnnotation::Bool);
    let symbol_t = Arc::new(CalcitTypeAnnotation::Symbol);
    let tag_t = Arc::new(CalcitTypeAnnotation::Tag);
    let cirru_quote_t = Arc::new(CalcitTypeAnnotation::CirruQuote);
    let list_dyn = Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Dynamic)));
    let ref_dyn = Arc::new(CalcitTypeAnnotation::Ref(Arc::new(CalcitTypeAnnotation::Dynamic)));

    match self {
      If => Some(SyntaxTypeSignature {
        param_names: vec!["condition", "then-expr", "else-expr"],
        param_types: vec![bool_t.clone(), dyn_t.clone(), dyn_t.clone()],
        return_type: dyn_t.clone(),
      }),
      Quote | Quasiquote => Some(SyntaxTypeSignature {
        param_names: vec!["expr"],
        param_types: vec![dyn_t.clone()],
        return_type: cirru_quote_t.clone(),
      }),
      Gensym => Some(SyntaxTypeSignature {
        param_names: vec!["name"],
        param_types: vec![symbol_t.clone()],
        return_type: symbol_t.clone(),
      }),
      Eval => Some(SyntaxTypeSignature {
        param_names: vec!["expr"],
        param_types: vec![cirru_quote_t.clone()],
        return_type: dyn_t.clone(),
      }),
      Macroexpand | Macroexpand1 | MacroexpandAll => Some(SyntaxTypeSignature {
        param_names: vec!["expr"],
        param_types: vec![cirru_quote_t.clone()],
        return_type: cirru_quote_t.clone(),
      }),
      Try => Some(SyntaxTypeSignature {
        param_names: vec!["expr", "err", "catch-expr"],
        param_types: vec![dyn_t.clone(), symbol_t.clone(), dyn_t.clone()],
        return_type: dyn_t.clone(),
      }),
      Defatom => Some(SyntaxTypeSignature {
        param_names: vec!["name", "init"],
        param_types: vec![symbol_t.clone(), dyn_t.clone()],
        return_type: symbol_t.clone(),
      }),
      Reset => Some(SyntaxTypeSignature {
        param_names: vec!["atom", "value"],
        param_types: vec![ref_dyn.clone(), dyn_t.clone()],
        return_type: dyn_t.clone(),
      }),
      HintFn => Some(SyntaxTypeSignature {
        param_names: vec!["hint", "f"],
        param_types: vec![tag_t.clone(), dyn_t.clone()],
        return_type: dyn_t.clone(),
      }),
      AssertType => Some(SyntaxTypeSignature {
        param_names: vec!["expr", "type"],
        param_types: vec![dyn_t.clone(), dyn_t.clone()],
        return_type: dyn_t.clone(),
      }),
      UnsafeCoerce => Some(SyntaxTypeSignature {
        param_names: vec!["value", "type"],
        param_types: vec![dyn_t.clone(), dyn_t.clone()],
        return_type: dyn_t.clone(),
      }),
      ParseCirruEdnAs => Some(SyntaxTypeSignature {
        param_names: vec!["text", "type"],
        param_types: vec![Arc::new(CalcitTypeAnnotation::String), dyn_t.clone()],
        return_type: dyn_t.clone(),
      }),
      AssertTraits => Some(SyntaxTypeSignature {
        param_names: vec!["expr", "trait"],
        param_types: vec![dyn_t.clone(), tag_t.clone()],
        return_type: dyn_t.clone(),
      }),
      CoreLet => Some(SyntaxTypeSignature {
        param_names: vec!["binding", "body"],
        param_types: vec![list_dyn.clone(), dyn_t.clone()],
        return_type: dyn_t.clone(),
      }),
      CallSpread => Some(SyntaxTypeSignature {
        param_names: vec!["f", "args"],
        param_types: vec![dyn_t.clone(), list_dyn.clone()],
        return_type: dyn_t.clone(),
      }),
      Match => Some(SyntaxTypeSignature {
        param_names: vec!["value", "branches"],
        param_types: vec![dyn_t.clone(), dyn_t.clone()],
        return_type: dyn_t.clone(),
      }),
      Defn | Defmacro | DefWasmExport | DefWasmImport | ArgSpread | ArgOptional | MacroInterpolate | MacroInterpolateSpread => None,
    }
  }
}
