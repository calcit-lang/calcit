use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter, EnumString};

use crate::calcit::CalcitTypeAnnotation;

#[derive(Debug, Clone)]
pub struct SyntaxTypeSignature {
  pub param_names: Vec<&'static str>,
  pub param_types: Vec<Arc<CalcitTypeAnnotation>>,
  pub return_type: Arc<CalcitTypeAnnotation>,
}

/// core syntax inside Calcit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, EnumIter, strum_macros::Display, AsRefStr, PartialOrd, Ord)]
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
  /// referenced state defined and attached to its namespace
  #[strum(serialize = "defref")]
  Defref,
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
  /// Check a JavaScript host object's declared external-trait member shape.
  #[strum(serialize = "js-cast")]
  JsCast,
  /// Parse Cirru EDN and deeply validate/construct the declared closed type.
  #[strum(serialize = "parse-cirru-edn-as")]
  ParseCirruEdnAs,
  /// Parse Cirru EDN into a closed type and return runtime failures as Result.
  #[strum(serialize = "try-parse-cirru-edn-as")]
  TryParseCirruEdnAs,
  /// Decode an evaluated Calcit Map into a declared typed data shape.
  #[strum(serialize = "decode-map-as")]
  DecodeMapAs,
  /// Decode an evaluated value into a closed type and return runtime failures as Result.
  #[strum(serialize = "try-decode-map-as")]
  TryDecodeMapAs,
  /// Encode a typed value into the open shape `decode-map-as` reads back.
  #[strum(serialize = "encode-map-as")]
  EncodeMapAs,
  /// Box a closed, monomorphic value into the closed `Any` existential container.
  #[strum(serialize = "any:box")]
  AnyBox,
  /// Downcast a closed `Any` box into `Option<TargetType>` by canonical `TypeId`.
  #[strum(serialize = "any:downcast")]
  AnyDowncast,
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

  pub fn get_type_signature(self) -> Option<SyntaxTypeSignature> {
    static SIGNATURES: LazyLock<HashMap<CalcitSyntax, SyntaxTypeSignature, foldhash::fast::FixedState>> = LazyLock::new(|| {
      use CalcitSyntax::*;
      let dyn_t = Arc::clone(&crate::calcit::DYNAMIC_TYPE);
      let bool_t = Arc::new(CalcitTypeAnnotation::Bool);
      let symbol_t = Arc::new(CalcitTypeAnnotation::Symbol);
      let tag_t = Arc::new(CalcitTypeAnnotation::Tag);
      let string_t = Arc::new(CalcitTypeAnnotation::String);
      let nil_t = Arc::new(CalcitTypeAnnotation::Nil);
      let cirru_quote_t = Arc::new(CalcitTypeAnnotation::CirruQuote);
      let any_t = Arc::new(CalcitTypeAnnotation::Any);
      let list_dyn = Arc::new(CalcitTypeAnnotation::List(Arc::clone(&dyn_t)));
      let value_t = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
      let ref_value_t = Arc::new(CalcitTypeAnnotation::Ref(Arc::clone(&value_t)));

      let mut map = HashMap::with_hasher(foldhash::fast::FixedState::default());
      for syntax in CalcitSyntax::iter() {
        let sig = match syntax {
          If => Some(SyntaxTypeSignature {
            param_names: vec!["condition", "then-expr", "else-expr"],
            param_types: vec![Arc::clone(&bool_t), Arc::clone(&dyn_t), Arc::clone(&dyn_t)],
            return_type: Arc::clone(&dyn_t),
          }),
          Quote | Quasiquote => Some(SyntaxTypeSignature {
            param_names: vec!["expr"],
            param_types: vec![Arc::clone(&dyn_t)],
            return_type: Arc::clone(&cirru_quote_t),
          }),
          Gensym => Some(SyntaxTypeSignature {
            param_names: vec!["name"],
            param_types: vec![Arc::clone(&symbol_t)],
            return_type: Arc::clone(&symbol_t),
          }),
          Eval => Some(SyntaxTypeSignature {
            param_names: vec!["expr"],
            param_types: vec![Arc::clone(&cirru_quote_t)],
            return_type: Arc::clone(&dyn_t),
          }),
          Macroexpand | Macroexpand1 | MacroexpandAll => Some(SyntaxTypeSignature {
            param_names: vec!["expr"],
            param_types: vec![Arc::clone(&cirru_quote_t)],
            return_type: Arc::clone(&cirru_quote_t),
          }),
          Try => Some(SyntaxTypeSignature {
            param_names: vec!["expr", "err", "catch-expr"],
            param_types: vec![Arc::clone(&dyn_t), Arc::clone(&symbol_t), Arc::clone(&dyn_t)],
            return_type: Arc::clone(&dyn_t),
          }),
          Defref => Some(SyntaxTypeSignature {
            param_names: vec!["name", "init"],
            param_types: vec![Arc::clone(&symbol_t), Arc::clone(&value_t)],
            return_type: Arc::clone(&ref_value_t),
          }),
          Reset => Some(SyntaxTypeSignature {
            param_names: vec!["ref", "value"],
            param_types: vec![Arc::clone(&ref_value_t), Arc::clone(&value_t)],
            return_type: Arc::clone(&value_t),
          }),
          HintFn => Some(SyntaxTypeSignature {
            param_names: vec!["target-or-schema", "schema"],
            param_types: vec![Arc::clone(&dyn_t), Arc::clone(&dyn_t)],
            return_type: Arc::clone(&nil_t),
          }),
          AssertType => Some(SyntaxTypeSignature {
            param_names: vec!["expr", "type"],
            param_types: vec![Arc::clone(&value_t), Arc::clone(&dyn_t)],
            return_type: Arc::clone(&value_t),
          }),
          UnsafeCoerce | JsCast | DecodeMapAs | TryDecodeMapAs | EncodeMapAs => Some(SyntaxTypeSignature {
            param_names: vec!["value", "type"],
            param_types: vec![Arc::clone(&dyn_t), Arc::clone(&dyn_t)],
            return_type: Arc::clone(&dyn_t),
          }),
          ParseCirruEdnAs | TryParseCirruEdnAs => Some(SyntaxTypeSignature {
            param_names: vec!["text", "type"],
            param_types: vec![Arc::clone(&string_t), Arc::clone(&dyn_t)],
            return_type: Arc::clone(&dyn_t),
          }),
          AnyBox => Some(SyntaxTypeSignature {
            param_names: vec!["value"],
            param_types: vec![Arc::clone(&value_t)],
            return_type: Arc::clone(&any_t),
          }),
          AnyDowncast => Some(SyntaxTypeSignature {
            param_names: vec!["box", "type"],
            param_types: vec![Arc::clone(&any_t), Arc::clone(&dyn_t)],
            return_type: Arc::clone(&dyn_t),
          }),
          AssertTraits => Some(SyntaxTypeSignature {
            param_names: vec!["expr", "trait"],
            param_types: vec![Arc::clone(&value_t), Arc::clone(&tag_t)],
            return_type: Arc::clone(&value_t),
          }),
          CoreLet => Some(SyntaxTypeSignature {
            param_names: vec!["binding", "body"],
            param_types: vec![Arc::clone(&list_dyn), Arc::clone(&dyn_t)],
            return_type: Arc::clone(&dyn_t),
          }),
          CallSpread => Some(SyntaxTypeSignature {
            param_names: vec!["f", "args"],
            param_types: vec![Arc::clone(&dyn_t), Arc::clone(&list_dyn)],
            return_type: Arc::clone(&dyn_t),
          }),
          Match => Some(SyntaxTypeSignature {
            param_names: vec!["value", "branches"],
            param_types: vec![Arc::clone(&dyn_t), Arc::clone(&dyn_t)],
            return_type: Arc::clone(&dyn_t),
          }),
          Defn | Defmacro | DefWasmExport | DefWasmImport | ArgSpread | ArgOptional | MacroInterpolate | MacroInterpolateSpread => None,
        };
        if let Some(sig) = sig {
          map.insert(syntax, sig);
        }
      }
      map
    });

    SIGNATURES.get(&self).cloned()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reset_signature_preserves_the_ref_value_type() {
    let signature = CalcitSyntax::Reset.get_type_signature().expect("reset! signature");
    let CalcitTypeAnnotation::Ref(inner) = signature.param_types[0].as_ref() else {
      panic!("reset! first parameter must be Ref<T>");
    };
    assert_eq!(inner, &signature.param_types[1]);
    assert_eq!(signature.return_type, signature.param_types[1]);
    assert!(matches!(
      signature.return_type.as_ref(),
      CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T"
    ));
  }

  #[test]
  fn assertion_signatures_preserve_the_checked_value_type() {
    for syntax in [CalcitSyntax::AssertType, CalcitSyntax::AssertTraits] {
      let signature = syntax.get_type_signature().expect("assertion signature");
      assert_eq!(signature.return_type, signature.param_types[0]);
      assert!(matches!(
        signature.return_type.as_ref(),
        CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T"
      ));
    }
  }
}
