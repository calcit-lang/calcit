//! Checked ownership and rollout classification for the Calcit language surface.
//!
//! These exhaustive matches are intentionally kept next to Calx lowering. Adding a
//! type or syntax variant must therefore make an explicit Calx ownership decision.
//! `CalcitProc` already supports iteration, so its edition count provides the same
//! review gate while the larger proc inventory is classified in staged groups.

use crate::calcit::{CalcitProc, CalcitSyntax, CalcitTypeAnnotation};

/// Which layer owns a Calcit language surface in the Calx program contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalxCoverageOwner {
  /// Resolved, expanded, or erased by Calcit before lowering.
  CompilerOnly,
  /// Implemented by typed lowering and the strict Calx VM.
  CalxCore,
  /// Supplied by the embedding host through an exact typed import.
  TypedHostImport,
  /// Valid static Calcit semantics scheduled for a later implementation slice.
  Planned,
  /// Not admitted by the program contract.
  Excluded,
}

impl CalxCoverageOwner {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::CompilerOnly => "compiler-only",
      Self::CalxCore => "calx-core",
      Self::TypedHostImport => "typed-host-import",
      Self::Planned => "planned",
      Self::Excluded => "excluded",
    }
  }
}

/// Rollout slice for a statically supported Calcit semantic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalxCoverageStage {
  /// Already supported by the strict kernel compatibility path.
  KernelFoundation,
  /// Program entry, `Unit`, strict text values, and typed imports.
  ProgramFoundation,
  /// Nominal struct/enum values, exhaustive match, Option, and Result.
  NominalData,
  /// Typed persistent List, Map, and Set values and operations.
  Collections,
  /// Statically typed functions, closures, higher-order calls, and arity features.
  TypedCallables,
  /// Explicitly selected reference, error, and other stateful semantics.
  StatefulCore,
  /// Compile-time, host-owned, or excluded surfaces have no VM rollout stage.
  NotApplicable,
}

/// One stable classification used by documentation, tests, and implementation planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalxCoverage {
  pub owner: CalxCoverageOwner,
  pub stage: CalxCoverageStage,
}

const fn coverage(owner: CalxCoverageOwner, stage: CalxCoverageStage) -> CalxCoverage {
  CalxCoverage { owner, stage }
}

const KERNEL: CalxCoverage = coverage(CalxCoverageOwner::CalxCore, CalxCoverageStage::KernelFoundation);
const PROGRAM: CalxCoverage = coverage(CalxCoverageOwner::Planned, CalxCoverageStage::ProgramFoundation);
const NOMINAL: CalxCoverage = coverage(CalxCoverageOwner::Planned, CalxCoverageStage::NominalData);
const COLLECTIONS: CalxCoverage = coverage(CalxCoverageOwner::Planned, CalxCoverageStage::Collections);
const CALLABLES: CalxCoverage = coverage(CalxCoverageOwner::Planned, CalxCoverageStage::TypedCallables);
const STATEFUL: CalxCoverage = coverage(CalxCoverageOwner::Planned, CalxCoverageStage::StatefulCore);
const COMPILER: CalxCoverage = coverage(CalxCoverageOwner::CompilerOnly, CalxCoverageStage::NotApplicable);
const HOST: CalxCoverage = coverage(CalxCoverageOwner::TypedHostImport, CalxCoverageStage::NotApplicable);
const EXCLUDED: CalxCoverage = coverage(CalxCoverageOwner::Excluded, CalxCoverageStage::NotApplicable);

/// Classifies every top-level type-annotation variant for the first Calx program roadmap.
///
/// This is an inventory decision, not an eligibility proof. Lowering must still
/// reject nested Dynamic, Nil, unresolved generics, and unsupported nominal fields.
pub fn classify_calx_type_surface(annotation: &CalcitTypeAnnotation) -> CalxCoverage {
  use CalcitTypeAnnotation::*;

  match annotation {
    Bool | Number | F64Buffer | Unit => KERNEL,
    String => coverage(CalxCoverageOwner::CalxCore, CalxCoverageStage::ProgramFoundation),
    Symbol | Tag | Buffer => PROGRAM,
    List(_) | Map(_, _) | Set(_) => COLLECTIONS,
    StructValue(_) | EnumValue(_) | Struct(_, _) | Enum(_, _) => NOMINAL,
    Fn(_) | Variadic(_) => CALLABLES,
    Ref(_) => STATEFUL,
    Macro(_) | Syntax(_) | StructDef(_) | EnumDef(_) | TypeVar(_) | TypeRef(_, _) | Trait(_) | TraitSet(_) | TypeSlot(_) => COMPILER,
    AnonymousEnum | DynFn | CirruQuote | Custom(_) | Dynamic | Optional(_) | JsNullish(_) | Nil | JsObject => EXCLUDED,
  }
}

/// Classifies every core syntax variant for the first Calx program-contract roadmap.
pub const fn classify_calx_syntax_surface(syntax: &CalcitSyntax) -> CalxCoverage {
  use CalcitSyntax::*;

  match syntax {
    If | CoreLet => KERNEL,
    Defn
    | Defmacro
    | Quasiquote
    | Gensym
    | Macroexpand
    | Macroexpand1
    | MacroexpandAll
    | MacroInterpolate
    | MacroInterpolateSpread
    | HintFn
    | AssertType
    | UnsafeCoerce
    | AssertTraits => COMPILER,
    Match => NOMINAL,
    CallSpread | ArgSpread | ArgOptional => CALLABLES,
    Try | Defatom | Reset => STATEFUL,
    ParseCirruEdnAs | TryParseCirruEdnAs | DecodeMapAs | TryDecodeMapAs => PROGRAM,
    DefWasmExport | DefWasmImport | Quote | Eval => EXCLUDED,
  }
}

/// Number of proc variants reviewed for the first checked coverage edition.
///
/// The test below intentionally fails when `CalcitProc` grows. The new variant
/// must be classified and this edition count updated in the same change.
pub const CALX_COVERAGE_PROC_COUNT_V1: usize = 234;

/// Classifies built-in operations. Most language-level operations are planned
/// until their typed value slice is selected; current kernel primitives and
/// obvious host capabilities are called out explicitly.
pub fn classify_calx_proc_surface(proc: CalcitProc) -> CalxCoverage {
  use CalcitProc::*;

  match proc {
    Recur | NativeEquals | NativeLessThan | NativeGreaterThan | NativeAdd | NativeMinus | NativeMultiply | NativeDivide
    | NativeF64ToI64Index | NativeF64BufferGet => KERNEL,
    GenerateId | GetEnv | NativeGetOs | CpuTime | UnixTimeMs | ReadFile | ReadDir | WriteFile | Quit | NativeDisplayStack => HOST,
    NativeResetGenSymIndex
    | NativeGetCalcitRunningMode
    | NativeGetCalcitBackend
    | RegisterCalcitBuiltinImpls
    | DeftypeSlot
    | WithTypeSlot => COMPILER,
    NativeGetDefDoc
    | NativeGetDefSchema
    | NativeExtractCodeIntoEdn
    | NativeDataToCode
    | NativeCirruType
    | NativeCirruNth
    | NativeCirruQuoteToList
    | NativeMethodsOf
    | NativeInspectMethods
    | NativeInspectType => EXCLUDED,
    _ => {
      let name = proc.as_ref();
      if name.starts_with("&list:")
        || name.starts_with("&map:")
        || name.starts_with("&set:")
        || name.starts_with("&buf-list:")
        || matches!(
          proc,
          List
            | Append
            | Prepend
            | Butlast
            | Range
            | Sort
            | Foldl
            | FoldlShortcut
            | FoldrShortcut
            | NativeListQ
            | NativeMap
            | NativeMerge
            | NativeMergeNonNil
            | ToPairs
            | Set
            | NativeInclude
            | NativeExclude
            | NativeDifference
            | NativeUnion
        )
      {
        COLLECTIONS
      } else if name.starts_with("&struct:")
        || name.starts_with("&struct-def:")
        || name.starts_with("&enum:")
        || name.starts_with("&enum-def:")
        || name.starts_with("&trait")
        || name.starts_with("&impl:")
        || matches!(
          proc,
          NativeEnum | NativeNamedEnumNew | NativeLooseStruct | NativeStruct | NativeStructPartial
        )
      {
        NOMINAL
      } else if matches!(proc, Atom | AtomDeref | AddWatch | RemoveWatch | Raise) {
        STATEFUL
      } else {
        PROGRAM
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use strum::IntoEnumIterator;

  use super::*;

  #[test]
  fn proc_coverage_edition_requires_review_when_the_enum_grows() {
    assert_eq!(CalcitProc::iter().count(), CALX_COVERAGE_PROC_COUNT_V1);
  }

  #[test]
  fn nil_and_dynamic_surfaces_stay_excluded() {
    for annotation in [CalcitTypeAnnotation::Nil, CalcitTypeAnnotation::Dynamic] {
      assert_eq!(classify_calx_type_surface(&annotation), EXCLUDED);
    }
    assert_eq!(classify_calx_syntax_surface(&CalcitSyntax::Eval), EXCLUDED);
  }

  #[test]
  fn external_effects_remain_typed_host_capabilities() {
    for proc in [
      CalcitProc::ReadFile,
      CalcitProc::WriteFile,
      CalcitProc::GetEnv,
      CalcitProc::UnixTimeMs,
    ] {
      assert_eq!(classify_calx_proc_surface(proc), HOST);
    }
  }

  #[test]
  fn value_families_follow_their_rollout_slices() {
    assert_eq!(
      classify_calx_type_surface(&CalcitTypeAnnotation::String),
      coverage(CalxCoverageOwner::CalxCore, CalxCoverageStage::ProgramFoundation)
    );
    for annotation in [CalcitTypeAnnotation::Tag, CalcitTypeAnnotation::Symbol] {
      assert_eq!(classify_calx_type_surface(&annotation), PROGRAM);
    }
    assert_eq!(classify_calx_proc_surface(CalcitProc::NativeListCount), COLLECTIONS);
    assert_eq!(classify_calx_proc_surface(CalcitProc::NativeMapAssoc), COLLECTIONS);
    assert_eq!(classify_calx_proc_surface(CalcitProc::NativeEnumNth), NOMINAL);
    assert_eq!(classify_calx_proc_surface(CalcitProc::AtomDeref), STATEFUL);
  }
}
