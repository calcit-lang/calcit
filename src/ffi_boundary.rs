//! Unified core vs. host-FFI boundary classification.
//!
//! Keeps host target identification (`Js`, `Native`, `Wasm`) in one module so
//! compiler passes, type checking, macro capability checks, and codegen do not
//! scatter duplicate platform-specific matches across core modules.

use std::fmt;

use crate::calcit::{CalcitSyntax, CalcitTypeAnnotation, MethodKind, NullableOrigin};

/// Host runtime target that owns a platform-specific syntax, method, or type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HostTarget {
  Js,
  Native,
  Wasm,
}

impl HostTarget {
  /// Returns the canonical lowercase target tag name.
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Js => "js",
      Self::Native => "native",
      Self::Wasm => "wasm",
    }
  }
}

impl fmt::Display for HostTarget {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

/// Returns `true` for JavaScript-only syntax and interop helper names recognized
/// during JS codegen and preprocessing.
pub fn is_js_syntax_procs(s: &str) -> bool {
  matches!(
    s,
    "aget"
      | "aset"
      | "js-get" // alias for aget
      | "js-set" // alias for aset
      | "js-delete"
      | "exists?"
      | "extract-cirru-edn"
      | "instance?"
      | "&js-object"
      | "js-array"
      | "js-await"
      | "load-console-formatter!"
      | "printable"
      | "new"
      | "set!"
      | "timeout-call"
      | "to-calcit-data"
      | "to-cirru-edn"
      | "to-js-data"
      | "&raw-code"
      | "js-for-await"
  )
}

impl CalcitSyntax {
  /// Returns the host target if this syntax form belongs to a host FFI boundary,
  /// or `None` when it is part of the portable core language.
  pub fn host_target(self) -> Option<HostTarget> {
    match self {
      Self::DefWasmExport | Self::DefWasmImport => Some(HostTarget::Wasm),
      Self::JsCast => Some(HostTarget::Js),
      Self::Defn
      | Self::Defmacro
      | Self::If
      | Self::CoreLet
      | Self::Quote
      | Self::Quasiquote
      | Self::Gensym
      | Self::Eval
      | Self::Macroexpand
      | Self::Macroexpand1
      | Self::MacroexpandAll
      | Self::Try
      | Self::Defref
      | Self::Reset
      | Self::HintFn
      | Self::CallSpread
      | Self::ArgSpread
      | Self::MacroInterpolate
      | Self::MacroInterpolateSpread
      | Self::AssertType
      | Self::UnsafeCoerce
      | Self::ParseCirruEdnAs
      | Self::TryParseCirruEdnAs
      | Self::DecodeMapAs
      | Self::TryDecodeMapAs
      | Self::EncodeMapAs
      | Self::AnyBox
      | Self::AnyDowncast
      | Self::AssertTraits
      | Self::DynBox
      | Self::Match => None,
    }
  }

  /// Returns `true` when this syntax form belongs to a host FFI boundary.
  pub fn is_host_ffi(self) -> bool {
    self.host_target().is_some()
  }
}

impl MethodKind {
  /// Returns the host target if this method kind performs host/external interop,
  /// or `None` for core method invocation and tag access.
  pub fn host_target(&self) -> Option<HostTarget> {
    match self {
      Self::Invoke(_) | Self::TagAccess => None,
      Self::InvokeNative
      | Self::InvokeNativeOptional
      | Self::ExternalAccess(_)
      | Self::ExternalGet(_)
      | Self::ExternalSet(_)
      | Self::ExternalInvoke(_)
      | Self::Access
      | Self::AccessOptional => Some(HostTarget::Js),
    }
  }

  /// Returns `true` when this method kind crosses a host FFI boundary.
  pub fn is_host_ffi(&self) -> bool {
    self.host_target().is_some()
  }
}

impl CalcitTypeAnnotation {
  /// Returns the host target if this type represents an opaque host boundary
  /// (`JsObject` on JS or `FfiResource` on Native), or `None` for core types.
  pub fn host_target(&self) -> Option<HostTarget> {
    match self {
      Self::JsObject => Some(HostTarget::Js),
      Self::FfiResource => Some(HostTarget::Native),
      Self::Never
      | Self::Bool
      | Self::Number
      | Self::Numeric(_)
      | Self::String
      | Self::Symbol
      | Self::Tag
      | Self::List(_)
      | Self::Tuple(_)
      | Self::Map(_, _)
      | Self::StructValue(_)
      | Self::EnumValue(_)
      | Self::AnonymousEnum
      | Self::DynFn
      | Self::Method
      | Self::Fn(_)
      | Self::Macro(_)
      | Self::Syntax(_)
      | Self::Set(_)
      | Self::Ref(_)
      | Self::Buffer
      | Self::BufList
      | Self::CirruQuote
      | Self::Variadic(_)
      | Self::Custom(_)
      | Self::Dynamic
      | Self::Nullable(_, NullableOrigin::Inferred | NullableOrigin::JsHost)
      | Self::Struct(_, _)
      | Self::Enum(_, _)
      | Self::StructDef(_)
      | Self::EnumDef(_)
      | Self::TypeVar(_)
      | Self::TypeRef(_, _)
      | Self::Trait(_)
      | Self::DynTrait(_)
      | Self::TraitSet(_)
      | Self::Nil
      | Self::Unit
      | Self::Any
      | Self::TypeSlot(_) => None,
    }
  }

  /// Returns `true` when this type represents an opaque host FFI value.
  pub fn is_host_ffi_type(&self) -> bool {
    self.host_target().is_some()
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use strum::IntoEnumIterator;

  use super::*;
  use crate::calcit::DYNAMIC_TYPE;

  #[test]
  fn syntax_host_target_partitions_every_variant() {
    let mut host_forms = Vec::new();
    let mut core_count = 0usize;
    for syntax in CalcitSyntax::iter() {
      match syntax.host_target() {
        Some(target) => {
          assert!(syntax.is_host_ffi());
          host_forms.push((syntax, target));
        }
        None => {
          assert!(!syntax.is_host_ffi());
          core_count += 1;
        }
      }
    }
    assert_eq!(
      host_forms,
      vec![
        (CalcitSyntax::DefWasmExport, HostTarget::Wasm),
        (CalcitSyntax::DefWasmImport, HostTarget::Wasm),
        (CalcitSyntax::JsCast, HostTarget::Js),
      ]
    );
    assert_eq!(core_count + host_forms.len(), CalcitSyntax::iter().count());
  }

  #[test]
  fn method_kind_host_target_partitions_core_and_host_variants() {
    let core_kinds = [MethodKind::Invoke(DYNAMIC_TYPE.clone()), MethodKind::TagAccess];
    for kind in &core_kinds {
      assert_eq!(kind.host_target(), None);
      assert!(!kind.is_host_ffi());
    }

    let host_kinds = [
      MethodKind::InvokeNative,
      MethodKind::InvokeNativeOptional,
      MethodKind::ExternalAccess(DYNAMIC_TYPE.clone()),
      MethodKind::ExternalGet(DYNAMIC_TYPE.clone()),
      MethodKind::ExternalSet(DYNAMIC_TYPE.clone()),
      MethodKind::ExternalInvoke(DYNAMIC_TYPE.clone()),
      MethodKind::Access,
      MethodKind::AccessOptional,
    ];
    for kind in &host_kinds {
      assert_eq!(kind.host_target(), Some(HostTarget::Js));
      assert!(kind.is_host_ffi());
    }
  }

  #[test]
  fn type_annotation_host_target_identifies_opaque_host_boundaries() {
    assert_eq!(CalcitTypeAnnotation::JsObject.host_target(), Some(HostTarget::Js));
    assert!(CalcitTypeAnnotation::JsObject.is_host_ffi_type());
    assert_eq!(CalcitTypeAnnotation::FfiResource.host_target(), Some(HostTarget::Native));
    assert!(CalcitTypeAnnotation::FfiResource.is_host_ffi_type());

    let core_types = [
      CalcitTypeAnnotation::Number,
      CalcitTypeAnnotation::String,
      CalcitTypeAnnotation::Bool,
      CalcitTypeAnnotation::Nil,
      CalcitTypeAnnotation::Unit,
      CalcitTypeAnnotation::Method,
      CalcitTypeAnnotation::Any,
      CalcitTypeAnnotation::Dynamic,
      CalcitTypeAnnotation::Nullable(Arc::new(CalcitTypeAnnotation::JsObject), NullableOrigin::JsHost),
      CalcitTypeAnnotation::Nullable(Arc::new(CalcitTypeAnnotation::Number), NullableOrigin::Inferred),
      CalcitTypeAnnotation::List(DYNAMIC_TYPE.clone()),
    ];
    for ty in &core_types {
      assert_eq!(ty.host_target(), None, "unexpected host target for {ty:?}");
      assert!(!ty.is_host_ffi_type());
    }
  }

  #[test]
  fn js_syntax_procs_excludes_core_syntaxes_and_procs() {
    for name in [
      "aget",
      "aset",
      "js-get",
      "js-set",
      "js-delete",
      "exists?",
      "extract-cirru-edn",
      "instance?",
      "&js-object",
      "js-array",
      "js-await",
      "load-console-formatter!",
      "printable",
      "new",
      "set!",
      "timeout-call",
      "to-calcit-data",
      "to-cirru-edn",
      "to-js-data",
      "&raw-code",
      "js-for-await",
    ] {
      assert!(is_js_syntax_procs(name), "expected JS syntax/proc: {name}");
    }
    for core in ["defn", "if", "let", "foldl", "=", "not=", "identical?", "&list:includes?"] {
      assert!(!is_js_syntax_procs(core), "core name leaked into JS procs: {core}");
    }
  }

  #[test]
  fn wasm_boundary_audit_excludes_js_host_constructs() {
    assert_eq!(CalcitSyntax::JsCast.host_target(), Some(HostTarget::Js));
    assert_eq!(CalcitTypeAnnotation::JsObject.host_target(), Some(HostTarget::Js));
    assert_eq!(MethodKind::InvokeNative.host_target(), Some(HostTarget::Js));
    assert_eq!(CalcitSyntax::DefWasmExport.host_target(), Some(HostTarget::Wasm));
    assert_eq!(CalcitSyntax::DefWasmImport.host_target(), Some(HostTarget::Wasm));
  }
}
