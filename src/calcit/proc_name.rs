use strum_macros::{AsRefStr, EnumString};

/// represent builtin functions for performance reasons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, EnumString, strum_macros::Display, AsRefStr)]
pub enum CalcitProc {
  // meta
  #[strum(serialize = "type-of")]
  TypeOf,
  #[strum(serialize = "recur")]
  Recur,
  #[strum(serialize = "format-to-lisp")]
  FormatToLisp,
  #[strum(serialize = "format-to-cirru")]
  FormatToCirru,
  #[strum(serialize = "&reset-gensym-index!")]
  NativeResetGenSymIndex,
  #[strum(serialize = "&get-calcit-running-mode")]
  NativeGetCalcitRunningMode,
  #[strum(serialize = "generate-id!")]
  GenerateId,
  #[strum(serialize = "turn-symbol")]
  TurnSymbol,
  #[strum(serialize = "turn-tag")]
  TurnTag,
  #[strum(serialize = "&compare")]
  NativeCompare,
  #[strum(serialize = "&get-os")]
  NativeGetOs,
  #[strum(serialize = "&format-ternary-tree")]
  NativeFormatTernaryTree,
  #[strum(serialize = "&buffer")]
  NativeBuffer,
  #[strum(serialize = "&hash")]
  NativeHash,
  #[strum(serialize = "&extract-code-into-edn")]
  NativeExtractCodeIntoEdn,
  #[strum(serialize = "&data-to-code")]
  NativeDataToCode,
  #[strum(serialize = "&cirru-type")]
  NativeCirruType,
  #[strum(serialize = "&cirru-nth")]
  NativeCirruNth,
  #[strum(serialize = "::")]
  NativeTuple,
  #[strum(serialize = "%::")]
  NativeEnumTupleNew,
  #[strum(serialize = "&tuple:nth")]
  NativeTupleNth,
  #[strum(serialize = "&tuple:assoc")]
  NativeTupleAssoc,
  #[strum(serialize = "&tuple:count")]
  NativeTupleCount,
  #[strum(serialize = "&tuple:impls")]
  NativeTupleImpls,
  #[strum(serialize = "&tuple:params")]
  NativeTupleParams,
  #[strum(serialize = "&tuple:enum")]
  NativeTupleEnum,
  #[strum(serialize = "&struct::new")]
  NativeStructNew,
  #[strum(serialize = "&enum::new")]
  NativeEnumNew,
  #[strum(serialize = "&trait::new")]
  NativeTraitNew,
  #[strum(serialize = "&record:impl-traits")]
  NativeRecordImplTraits,
  #[strum(serialize = "&tuple:impl-traits")]
  NativeTupleImplTraits,
  #[strum(serialize = "&struct:impl-traits")]
  NativeStructImplTraits,
  #[strum(serialize = "&enum:impl-traits")]
  NativeEnumImplTraits,
  #[strum(serialize = "&tuple:enum-has-variant?")]
  NativeTupleEnumHasVariant,
  #[strum(serialize = "&tuple:enum-variant-arity")]
  NativeTupleEnumVariantArity,
  #[strum(serialize = "&tuple:validate-enum")]
  NativeTupleValidateEnum,
  #[strum(serialize = "&display-stack")]
  NativeDisplayStack,
  #[strum(serialize = "&inspect-impl-methods")]
  NativeInspectImplMethods,
  #[strum(serialize = "&inspect-type")]
  NativeInspectType,
  #[strum(serialize = "&assert-traits")]
  NativeAssertTraits,
  #[strum(serialize = "raise")]
  Raise,
  #[strum(serialize = "quit!")]
  Quit,
  #[strum(serialize = "get-env")]
  GetEnv,
  #[strum(serialize = "&get-calcit-backend")]
  NativeGetCalcitBackend,
  #[strum(serialize = "register-calcit-builtin-impls")]
  RegisterCalcitBuiltinImpls,
  #[strum(serialize = "read-file")]
  ReadFile,
  #[strum(serialize = "write-file")]
  WriteFile,
  /// to detect syntax `&`
  #[strum(serialize = "is-spreading-mark?")]
  IsSpreadingMark,
  // external format
  #[strum(serialize = "parse-cirru")]
  ParseCirru,
  #[strum(serialize = "parse-cirru-list")]
  ParseCirruList,
  #[strum(serialize = "format-cirru")]
  FormatCirru,
  #[strum(serialize = "format-cirru-one-liner")]
  FormatCirruOneLiner,
  #[strum(serialize = "parse-cirru-edn")]
  ParseCirruEdn,
  #[strum(serialize = "format-cirru-edn")]
  FormatCirruEdn,
  #[strum(serialize = "&cirru-quote:to-list")]
  NativeCirruQuoteToList,
  // time
  #[strum(serialize = "cpu-time")]
  CpuTime,
  // logics
  #[strum(serialize = "&=")]
  NativeEquals,
  #[strum(serialize = "&<")]
  NativeLessThan,
  #[strum(serialize = "&>")]
  NativeGreaterThan,
  #[strum(serialize = "not")]
  Not,
  #[strum(serialize = "identical?")]
  Identical,
  // math
  #[strum(serialize = "&+")]
  NativeAdd,
  #[strum(serialize = "&-")]
  NativeMinus,
  #[strum(serialize = "&*")]
  NativeMultiply,
  #[strum(serialize = "&/")]
  NativeDivide,
  #[strum(serialize = "round")]
  Round,
  #[strum(serialize = "floor")]
  Floor,
  #[strum(serialize = "sin")]
  Sin,
  #[strum(serialize = "cos")]
  Cos,
  #[strum(serialize = "pow")]
  Pow,
  #[strum(serialize = "ceil")]
  Ceil,
  #[strum(serialize = "sqrt")]
  Sqrt,
  #[strum(serialize = "round?")]
  IsRound,
  #[strum(serialize = "&number:fract")]
  NativeNumberFract,
  #[strum(serialize = "&number:rem")]
  NativeNumberRem,
  #[strum(serialize = "&number:format")]
  NativeNumberFormat,
  #[strum(serialize = "&number:display-by")]
  NativeNumberDisplayBy,
  #[strum(serialize = "bit-shl")]
  BitShl,
  #[strum(serialize = "bit-shr")]
  BitShr,
  #[strum(serialize = "bit-and")]
  BitAnd,
  #[strum(serialize = "bit-or")]
  BitOr,
  #[strum(serialize = "bit-xor")]
  BitXor,
  #[strum(serialize = "bit-not")]
  BitNot,
  // strings
  #[strum(serialize = "&str:concat")]
  NativeStrConcat,
  #[strum(serialize = "trim")]
  Trim,
  #[strum(serialize = "&str")]
  NativeStr,
  #[strum(serialize = "turn-string")]
  TurnString,
  #[strum(serialize = "split")]
  Split,
  #[strum(serialize = "split-lines")]
  SplitLines,
  #[strum(serialize = "starts-with?")]
  StartsWith,
  #[strum(serialize = "ends-with?")]
  EndsWith,
  #[strum(serialize = "get-char-code")]
  GetCharCode,
  #[strum(serialize = "char-from-code")]
  CharFromCode,
  #[strum(serialize = "to-lispy-string")]
  PrStr,
  #[strum(serialize = "parse-float")]
  ParseFloat,
  #[strum(serialize = "blank?")]
  IsBlank,
  #[strum(serialize = "&str:compare")]
  NativeStrCompare,
  #[strum(serialize = "&str:replace")]
  NativeStrReplace,
  #[strum(serialize = "&str:slice")]
  NativeStrSlice,
  #[strum(serialize = "&str:find-index")]
  NativeStrFindIndex,
  #[strum(serialize = "&str:escape")]
  NativeStrEscape,
  #[strum(serialize = "&str:count")]
  NativeStrCount,
  #[strum(serialize = "&str:empty?")]
  NativeStrEmpty,
  #[strum(serialize = "&str:contains?")]
  NativeStrContains,
  #[strum(serialize = "&str:includes?")]
  NativeStrIncludes,
  #[strum(serialize = "&str:nth")]
  NativeStrNth,
  #[strum(serialize = "&str:first")]
  NativeStrFirst,
  #[strum(serialize = "&str:rest")]
  NativeStrRest,
  #[strum(serialize = "&str:pad-left")]
  NativeStrPadLeft,
  #[strum(serialize = "&str:pad-right")]
  NativeStrPadRight,
  // lists
  #[strum(serialize = "[]")]
  List,
  #[strum(serialize = "append")]
  Append,
  #[strum(serialize = "prepend")]
  Prepend,
  #[strum(serialize = "butlast")]
  Butlast,
  #[strum(serialize = "range")]
  Range,
  #[strum(serialize = "sort")]
  Sort,
  #[strum(serialize = "foldl")]
  Foldl,
  #[strum(serialize = "foldl-shortcut")]
  FoldlShortcut,
  #[strum(serialize = "foldr-shortcut")]
  FoldrShortcut,
  #[strum(serialize = "&list:reverse")]
  NativeListReverse,
  #[strum(serialize = "&list:concat")]
  NativeListConcat,
  #[strum(serialize = "&list:count")]
  NativeListCount,
  #[strum(serialize = "&list:empty?")]
  NativeListEmpty,
  #[strum(serialize = "&list:slice")]
  NativeListSlice,
  #[strum(serialize = "&list:assoc-before")]
  NativeListAssocBefore,
  #[strum(serialize = "&list:assoc-after")]
  NativeListAssocAfter,
  #[strum(serialize = "&list:contains?")]
  NativeListContains,
  #[strum(serialize = "&list:includes?")]
  NativeListIncludes,
  #[strum(serialize = "&list:nth")]
  NativeListNth,
  #[strum(serialize = "&list:first")]
  NativeListFirst,
  #[strum(serialize = "&list:rest")]
  NativeListRest,
  #[strum(serialize = "&list:assoc")]
  NativeListAssoc,
  #[strum(serialize = "&list:dissoc")]
  NativeListDissoc,
  #[strum(serialize = "&list:to-set")]
  NativeListToSet,
  #[strum(serialize = "&list:distinct")]
  NativeListDistinct,
  // maps
  #[strum(serialize = "&{}")]
  NativeMap,
  #[strum(serialize = "&merge")]
  NativeMerge,
  #[strum(serialize = "to-pairs")]
  ToPairs,
  #[strum(serialize = "&merge-non-nil")]
  NativeMergeNonNil,
  #[strum(serialize = "&map:get")]
  NativeMapGet,
  #[strum(serialize = "&map:dissoc")]
  NativeMapDissoc,
  #[strum(serialize = "&map:to-list")]
  NativeMapToList,
  #[strum(serialize = "&map:count")]
  NativeMapCount,
  #[strum(serialize = "&map:empty?")]
  NativeMapEmpty,
  #[strum(serialize = "&map:contains?")]
  NativeMapContains,
  #[strum(serialize = "&map:includes?")]
  NativeMapIncludes,
  #[strum(serialize = "&map:destruct")]
  NativeMapDestruct,
  #[strum(serialize = "&map:assoc")]
  NativeMapAssoc,
  #[strum(serialize = "&map:diff-new")]
  NativeMapDiffNew,
  #[strum(serialize = "&map:diff-keys")]
  NativeMapDiffKeys,
  #[strum(serialize = "&map:common-keys")]
  NativeMapCommonKeys,
  // sets
  #[strum(serialize = "#{}")]
  Set,
  #[strum(serialize = "&include")]
  NativeInclude,
  #[strum(serialize = "&exclude")]
  NativeExclude,
  #[strum(serialize = "&difference")]
  NativeDifference,
  #[strum(serialize = "&union")]
  NativeUnion,
  #[strum(serialize = "&set:intersection")]
  NativeSetIntersection,
  #[strum(serialize = "&set:to-list")]
  NativeSetToList,
  #[strum(serialize = "&set:count")]
  NativeSetCount,
  #[strum(serialize = "&set:empty?")]
  NativeSetEmpty,
  #[strum(serialize = "&set:includes?")]
  NativeSetIncludes,
  #[strum(serialize = "&set:destruct")]
  NativeSetDestruct,
  // refs
  #[strum(serialize = "atom")]
  Atom,
  #[strum(serialize = "&atom:deref")]
  AtomDeref,
  #[strum(serialize = "add-watch")]
  AddWatch,
  #[strum(serialize = "remove-watch")]
  RemoveWatch,
  // records
  #[strum(serialize = "new-record")]
  NewRecord,
  #[strum(serialize = "new-impl-record")]
  NewImplRecord,
  #[strum(serialize = "&%{}")]
  NativeRecord,
  #[strum(serialize = "&record:with")]
  NativeRecordWith,
  #[strum(serialize = "&record:impls")]
  NativeRecordImpls,
  #[strum(serialize = "&record:matches?")]
  NativeRecordMatches,
  #[strum(serialize = "&record:from-map")]
  NativeRecordFromMap,
  #[strum(serialize = "&record:get-name")]
  NativeRecordGetName,
  #[strum(serialize = "&record:to-map")]
  NativeRecordToMap,
  #[strum(serialize = "&record:count")]
  NativeRecordCount,
  #[strum(serialize = "&record:contains?")]
  NativeRecordContains,
  #[strum(serialize = "&record:get")]
  NativeRecordGet,
  #[strum(serialize = "&record:assoc")]
  NativeRecordAssoc,
  #[strum(serialize = "&record:extend-as")]
  NativeRecordExtendAs,
}

use crate::CalcitTypeAnnotation;
use std::sync::Arc;

/// Type signature for a Proc (builtin function)
#[derive(Debug, Clone)]
pub struct ProcTypeSignature {
  /// return type declared
  pub return_type: Arc<CalcitTypeAnnotation>,
  /// argument types in order. Use Variadic to mark variadic args (no checking after this mark)
  pub arg_types: Vec<Arc<CalcitTypeAnnotation>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcArity {
  pub min: usize,
  pub max: Option<usize>,
}

impl ProcTypeSignature {
  pub fn arity(&self) -> ProcArity {
    let mut min = 0;
    let mut max = 0;
    let mut has_variadic = false;

    for t in &self.arg_types {
      match t.as_ref() {
        CalcitTypeAnnotation::Variadic(_) => {
          has_variadic = true;
          break;
        }
        CalcitTypeAnnotation::Optional(_) => {
          max += 1;
        }
        _ => {
          min += 1;
          max += 1;
        }
      }
    }

    ProcArity {
      min,
      max: if has_variadic { None } else { Some(max) },
    }
  }
}

fn tag_type(name: &str) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::from_tag_name(name))
}

fn some_tag(name: &str) -> Arc<CalcitTypeAnnotation> {
  tag_type(name)
}

fn some_set() -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Set(dynamic_tag()))
}

fn optional_tag(name: &str) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Optional(tag_type(name)))
}

fn optional_dynamic() -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Optional(dynamic_tag()))
}

fn dynamic_tag() -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Dynamic)
}

fn variadic_dynamic() -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Variadic(dynamic_tag()))
}

impl CalcitProc {
  /// Get the namespace and definition name for this proc.
  /// All built-in procs are defined in calcit.core namespace.
  /// Returns (namespace, definition_name)
  pub fn get_ns_def(&self) -> (&'static str, &str) {
    ("calcit.core", self.as_ref())
  }

  /// Get the type signature for this proc if available
  /// Returns None for procs without type annotations
  pub fn get_type_signature(&self) -> Option<ProcTypeSignature> {
    use CalcitProc::*;

    match self {
      // === Meta operations ===
      TypeOf => Some(ProcTypeSignature {
        return_type: some_tag("tag"),
        arg_types: vec![dynamic_tag()],
      }),
      FormatToLisp | FormatToCirru => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![dynamic_tag()],
      }),
      TurnSymbol => Some(ProcTypeSignature {
        return_type: some_tag("symbol"),
        arg_types: vec![some_tag("string")],
      }),
      TurnTag => Some(ProcTypeSignature {
        return_type: some_tag("tag"),
        arg_types: vec![some_tag("string")],
      }),
      NativeCompare => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![dynamic_tag(), dynamic_tag()],
      }),
      NativeGetOs => Some(ProcTypeSignature {
        return_type: some_tag("tag"),
        arg_types: vec![],
      }),
      NativeHash => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![dynamic_tag()],
      }),
      GenerateId => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![optional_tag("number"), optional_tag("string")],
      }),
      NativeGetCalcitRunningMode => Some(ProcTypeSignature {
        return_type: some_tag("tag"),
        arg_types: vec![],
      }),
      NativeGetCalcitBackend => Some(ProcTypeSignature {
        return_type: some_tag("tag"),
        arg_types: vec![],
      }),
      NativeDisplayStack => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![variadic_dynamic()],
      }),
      NativeAssertTraits => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![dynamic_tag(), some_tag("trait")],
      }),
      NativeCirruType => Some(ProcTypeSignature {
        return_type: some_tag("tag"),
        arg_types: vec![dynamic_tag()],
      }),
      NativeResetGenSymIndex => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![],
      }),
      NativeInspectImplMethods => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag(), optional_tag("string")],
      }),
      NativeInspectType => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag(), optional_tag("tag")],
      }),
      NativeExtractCodeIntoEdn => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag()],
      }),
      NativeDataToCode => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag()],
      }),
      IsSpreadingMark => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),

      // === Math operations ===
      NativeAdd => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("number"), some_tag("number")],
      }),
      NativeMinus | NativeMultiply | NativeDivide | Pow | NativeNumberRem => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("number"), some_tag("number")],
      }),
      Floor | Ceil | Round | Sin | Cos | Sqrt | NativeNumberFract => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("number")],
      }),
      IsRound => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("number")],
      }),
      BitShl | BitShr | BitAnd | BitOr | BitXor => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("number"), some_tag("number")],
      }),
      BitNot => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("number")],
      }),
      NativeNumberFormat => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("number"), some_tag("number")],
      }),
      NativeNumberDisplayBy => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("number"), some_tag("number")],
      }),

      // === Comparison & Logic ===
      NativeLessThan | NativeGreaterThan => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("number"), some_tag("number")],
      }),
      NativeEquals | Identical => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag(), dynamic_tag()],
      }),
      Not => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![optional_tag("bool")],
      }),

      // === String operations ===
      NativeStrConcat => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![dynamic_tag(), dynamic_tag()],
      }),
      Trim => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("string"), optional_tag("string")],
      }),
      TurnString => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![dynamic_tag()],
      }),
      NativeStr => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![variadic_dynamic()],
      }),
      Split => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("string"), some_tag("string")],
      }),
      SplitLines => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("string")],
      }),
      StartsWith | EndsWith => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag(), dynamic_tag()],
      }),
      GetCharCode => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("string")],
      }),
      CharFromCode => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("number")],
      }),
      PrStr => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![dynamic_tag()],
      }),
      ParseFloat => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("string")],
      }),
      IsBlank => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("string")],
      }),
      NativeStrCompare => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("string"), some_tag("string")],
      }),
      NativeStrReplace => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("string"), some_tag("string"), some_tag("string")],
      }),
      NativeStrSlice => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("string"), some_tag("number"), optional_tag("number")],
      }),
      NativeStrFindIndex => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("string"), some_tag("string")],
      }),
      NativeStrEscape => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("string")],
      }),
      NativeStrCount => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("string")],
      }),
      NativeStrEmpty => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("string")],
      }),
      NativeStrContains | NativeStrIncludes => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("string"), some_tag("string")],
      }),
      NativeStrNth => Some(ProcTypeSignature {
        return_type: optional_tag("string"),
        arg_types: vec![some_tag("string"), some_tag("number")],
      }),
      NativeStrFirst => Some(ProcTypeSignature {
        return_type: optional_tag("string"),
        arg_types: vec![some_tag("string")],
      }),
      NativeStrRest => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("string")],
      }),
      NativeStrPadLeft | NativeStrPadRight => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("string"), some_tag("number"), some_tag("string")],
      }),

      // === List operations ===
      List => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![variadic_dynamic()],
      }),
      Append | Prepend => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("list"), dynamic_tag()],
      }),
      Butlast | NativeListReverse => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("list")],
      }),
      Range => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("number"), optional_tag("number"), optional_tag("number")],
      }),
      Sort => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("list"), dynamic_tag()],
      }),
      NativeListConcat => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![variadic_dynamic()],
      }),
      NativeListCount => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("list")],
      }),
      NativeListEmpty => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("list")],
      }),
      NativeListContains | NativeListIncludes => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("list"), dynamic_tag()],
      }),
      NativeListSlice => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("list"), some_tag("number"), optional_tag("number")],
      }),
      NativeListNth => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("list"), some_tag("number")],
      }),
      NativeListFirst => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("list")],
      }),
      NativeListRest => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("list")],
      }),
      NativeListAssoc | NativeListAssocBefore | NativeListAssocAfter => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("list"), some_tag("number"), dynamic_tag()],
      }),
      NativeListDissoc => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("list"), some_tag("number")],
      }),
      NativeListToSet => Some(ProcTypeSignature {
        return_type: some_set(),
        arg_types: vec![some_tag("list")],
      }),
      NativeListDistinct => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("list")],
      }),
      Foldl => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag(), dynamic_tag(), dynamic_tag()],
      }),
      FoldlShortcut | FoldrShortcut => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag(), dynamic_tag(), dynamic_tag(), dynamic_tag()],
      }),

      // === Map operations ===
      NativeMap => Some(ProcTypeSignature {
        return_type: some_tag("map"),
        arg_types: vec![variadic_dynamic()],
      }),
      NativeMerge | NativeMergeNonNil => Some(ProcTypeSignature {
        return_type: some_tag("map"),
        arg_types: vec![some_tag("map"), some_tag("map")],
      }),
      ToPairs => Some(ProcTypeSignature {
        return_type: some_set(),
        arg_types: vec![some_tag("map")],
      }),
      NativeMapToList => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("map")],
      }),
      NativeMapGet => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("map"), dynamic_tag()],
      }),
      NativeMapDissoc => Some(ProcTypeSignature {
        return_type: some_tag("map"),
        arg_types: vec![some_tag("map"), dynamic_tag(), variadic_dynamic()],
      }),
      NativeMapCount => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("map")],
      }),
      NativeMapEmpty => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("map")],
      }),
      NativeMapContains | NativeMapIncludes => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("map"), dynamic_tag()],
      }),
      NativeMapAssoc => Some(ProcTypeSignature {
        return_type: some_tag("map"),
        arg_types: vec![some_tag("map"), dynamic_tag(), dynamic_tag(), variadic_dynamic()],
      }),
      NativeMapDiffNew => Some(ProcTypeSignature {
        return_type: some_tag("map"),
        arg_types: vec![some_tag("map"), some_tag("map")],
      }),
      NativeMapDiffKeys | NativeMapCommonKeys => Some(ProcTypeSignature {
        return_type: some_set(),
        arg_types: vec![some_tag("map"), some_tag("map")],
      }),
      NativeMapDestruct => Some(ProcTypeSignature {
        return_type: optional_tag("list"),
        arg_types: vec![some_tag("map")],
      }),

      // === Set operations ===
      Set => Some(ProcTypeSignature {
        return_type: some_set(),
        arg_types: vec![variadic_dynamic()],
      }),
      NativeInclude | NativeExclude => Some(ProcTypeSignature {
        return_type: some_set(),
        arg_types: vec![some_set(), dynamic_tag()],
      }),
      NativeDifference | NativeUnion | NativeSetIntersection => Some(ProcTypeSignature {
        return_type: some_set(),
        arg_types: vec![some_set(), some_set()],
      }),
      NativeSetToList => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_set()],
      }),
      NativeSetCount => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_set()],
      }),
      NativeSetEmpty => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_set()],
      }),
      NativeSetIncludes => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_set(), dynamic_tag()],
      }),
      NativeSetDestruct => Some(ProcTypeSignature {
        return_type: optional_tag("list"),
        arg_types: vec![some_set()],
      }),

      // === Tuple operations ===
      NativeTuple => Some(ProcTypeSignature {
        return_type: some_tag("tuple"),
        arg_types: vec![dynamic_tag(), variadic_dynamic()],
      }),
      NativeEnumTupleNew => Some(ProcTypeSignature {
        return_type: some_tag("tuple"),
        arg_types: vec![dynamic_tag(), some_tag("tag"), variadic_dynamic()],
      }),
      NativeTupleNth => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("tuple"), some_tag("number")],
      }),
      NativeTupleAssoc => Some(ProcTypeSignature {
        return_type: some_tag("tuple"),
        arg_types: vec![some_tag("tuple"), some_tag("number"), dynamic_tag()],
      }),
      NativeTupleCount => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("tuple")],
      }),
      NativeTupleImpls => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("tuple")],
      }),
      NativeTupleParams => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("tuple")],
      }),
      NativeTupleEnum => Some(ProcTypeSignature {
        return_type: optional_tag("enum"),
        arg_types: vec![some_tag("tuple")],
      }),
      NativeStructNew => Some(ProcTypeSignature {
        return_type: some_tag("struct"),
        arg_types: vec![some_tag("tag"), variadic_dynamic()],
      }),
      NativeEnumNew => Some(ProcTypeSignature {
        return_type: some_tag("enum"),
        arg_types: vec![some_tag("tag"), variadic_dynamic()],
      }),
      NativeTraitNew => Some(ProcTypeSignature {
        return_type: some_tag("trait"),
        arg_types: vec![dynamic_tag(), some_tag("list")],
      }),
      NativeRecordImplTraits => Some(ProcTypeSignature {
        return_type: some_tag("record"),
        arg_types: vec![some_tag("record"), variadic_dynamic()],
      }),
      NativeTupleImplTraits => Some(ProcTypeSignature {
        return_type: some_tag("tuple"),
        arg_types: vec![some_tag("tuple"), variadic_dynamic()],
      }),
      NativeStructImplTraits => Some(ProcTypeSignature {
        return_type: some_tag("struct"),
        arg_types: vec![some_tag("struct"), variadic_dynamic()],
      }),
      NativeEnumImplTraits => Some(ProcTypeSignature {
        return_type: some_tag("enum"),
        arg_types: vec![some_tag("enum"), variadic_dynamic()],
      }),
      NativeTupleEnumHasVariant => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("enum"), some_tag("tag")],
      }),
      NativeTupleEnumVariantArity => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("enum"), some_tag("tag")],
      }),
      NativeTupleValidateEnum => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![some_tag("tuple"), some_tag("tag")],
      }),

      // === Record operations ===
      NativeRecord => Some(ProcTypeSignature {
        return_type: some_tag("record"),
        arg_types: vec![some_tag("record"), variadic_dynamic()],
      }),
      NativeRecordWith => Some(ProcTypeSignature {
        return_type: some_tag("record"),
        arg_types: vec![some_tag("record"), dynamic_tag(), dynamic_tag(), variadic_dynamic()],
      }),
      NativeRecordAssoc => Some(ProcTypeSignature {
        return_type: some_tag("record"),
        arg_types: vec![some_tag("record"), dynamic_tag(), dynamic_tag()],
      }),
      NativeRecordGet => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("record"), some_tag("tag")],
      }),
      NativeRecordCount => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("record")],
      }),
      NativeRecordContains => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("record"), dynamic_tag()],
      }),
      NativeRecordMatches => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("record"), dynamic_tag()],
      }),
      NativeRecordToMap => Some(ProcTypeSignature {
        return_type: some_tag("map"),
        arg_types: vec![some_tag("record")],
      }),
      NativeRecordFromMap => Some(ProcTypeSignature {
        return_type: some_tag("record"),
        arg_types: vec![some_tag("record"), some_tag("map")],
      }),
      NativeRecordGetName => Some(ProcTypeSignature {
        return_type: some_tag("tag"),
        arg_types: vec![some_tag("record")],
      }),
      NewRecord | NewImplRecord => Some(ProcTypeSignature {
        return_type: some_tag("record"),
        arg_types: vec![some_tag("tag"), variadic_dynamic()],
      }),
      NativeRecordImpls => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("record")],
      }),
      NativeRecordExtendAs => Some(ProcTypeSignature {
        return_type: some_tag("record"),
        arg_types: vec![some_tag("record"), some_tag("tag"), some_tag("tag"), dynamic_tag()],
      }),

      // === Refs/Atoms ===
      Atom => Some(ProcTypeSignature {
        return_type: some_tag("ref"),
        arg_types: vec![dynamic_tag()],
      }),
      AtomDeref => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("ref")],
      }),
      AddWatch => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![some_tag("ref"), dynamic_tag(), dynamic_tag()],
      }),
      RemoveWatch => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![some_tag("ref"), dynamic_tag()],
      }),

      // === I/O operations ===
      ReadFile => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("string")],
      }),
      WriteFile => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![some_tag("string"), some_tag("string")],
      }),
      Raise => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![variadic_dynamic()],
      }),
      Quit => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![some_tag("number")],
      }),
      GetEnv => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("string"), optional_dynamic()],
      }),

      // === Time ===
      CpuTime => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![],
      }),

      // === Cirru format ===
      ParseCirru => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("string")],
      }),
      ParseCirruEdn => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("string"), optional_dynamic()],
      }),
      FormatCirru | FormatCirruEdn => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![dynamic_tag(), optional_tag("bool")],
      }),
      FormatCirruOneLiner => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![dynamic_tag()],
      }),
      ParseCirruList | NativeCirruQuoteToList => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![dynamic_tag()],
      }),
      NativeCirruNth => Some(ProcTypeSignature {
        return_type: some_tag("cirru-quote"),
        arg_types: vec![some_tag("cirru-quote"), some_tag("number")],
      }),

      // === Buffer ===
      NativeBuffer => Some(ProcTypeSignature {
        return_type: some_tag("buffer"),
        arg_types: vec![variadic_dynamic()],
      }),
      NativeFormatTernaryTree => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("list")],
      }),
      RegisterCalcitBuiltinImpls => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![dynamic_tag()],
      }),

      // === Special forms and control flow ===
      // These typically don't have simple type signatures or are handled specially
      Recur => None,
    }
  }

  /// Check if this proc has a type signature
  pub fn has_type_signature(&self) -> bool {
    self.get_type_signature().is_some()
  }
}
