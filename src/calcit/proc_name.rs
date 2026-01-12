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
  NativeClassTuple,
  #[strum(serialize = "%%::")]
  NativeEnumTuple,
  #[strum(serialize = "&tuple:nth")]
  NativeTupleNth,
  #[strum(serialize = "&tuple:assoc")]
  NativeTupleAssoc,
  #[strum(serialize = "&tuple:count")]
  NativeTupleCount,
  #[strum(serialize = "&tuple:class")]
  NativeTupleClass,
  #[strum(serialize = "&tuple:params")]
  NativeTupleParams,
  #[strum(serialize = "&tuple:with-class")]
  NativeTupleWithClass,
  #[strum(serialize = "&display-stack")]
  NativeDisplayStack,
  #[strum(serialize = "raise")]
  Raise,
  #[strum(serialize = "quit!")]
  Quit,
  #[strum(serialize = "get-env")]
  GetEnv,
  #[strum(serialize = "&get-calcit-backend")]
  NativeGetCalcitBackend,
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
  #[strum(serialize = "new-class-record")]
  NewClassRecord,
  #[strum(serialize = "&%{}")]
  NativeRecord,
  #[strum(serialize = "&record:with")]
  NativeRecordWith,
  #[strum(serialize = "&record:class")]
  NativeRecordClass,
  #[strum(serialize = "&record:with-class")]
  NativeRecordWithClass,
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

use crate::Calcit;
use std::sync::Arc;

/// Type signature for a Proc (builtin function)
#[derive(Debug, Clone)]
pub struct ProcTypeSignature {
  /// return type declared
  pub return_type: Option<Arc<Calcit>>,
  /// argument types in order. Use tag("&") to mark variadic args (no checking after this mark)
  pub arg_types: Vec<Option<Arc<Calcit>>>,
}

impl CalcitProc {
  /// Get the type signature for this proc if available
  /// Returns None for procs without type annotations
  pub fn get_type_signature(&self) -> Option<ProcTypeSignature> {
    use CalcitProc::*;

    match self {
      // === Meta operations ===
      TypeOf => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("tag"))),
        arg_types: vec![None],
      }),
      FormatToLisp | FormatToCirru => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![None],
      }),
      TurnSymbol => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("symbol"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string")))],
      }),
      TurnTag => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("tag"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string")))],
      }),
      NativeCompare => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![None, None],
      }),
      NativeGetOs => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("tag"))),
        arg_types: vec![],
      }),
      NativeHash => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![None],
      }),
      GenerateId => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![],
      }),
      NativeGetCalcitRunningMode => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("tag"))),
        arg_types: vec![],
      }),
      NativeGetCalcitBackend => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("tag"))),
        arg_types: vec![],
      }),
      NativeDisplayStack => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("nil"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("&")))],
      }),
      NativeCirruType => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("tag"))),
        arg_types: vec![None],
      }),

      // === Math operations ===
      NativeAdd | NativeMinus | NativeMultiply | NativeDivide | Pow | NativeNumberRem => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("number"))), Some(Arc::new(Calcit::tag("number")))],
      }),
      Floor | Ceil | Round | Sin | Cos | Sqrt | NativeNumberFract => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("number")))],
      }),
      IsRound => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("number")))],
      }),
      BitShl | BitShr | BitAnd | BitOr | BitXor => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("number"))), Some(Arc::new(Calcit::tag("number")))],
      }),
      BitNot => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("number")))],
      }),
      NativeNumberFormat => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("number")))],
      }),

      // === Comparison & Logic ===
      NativeEquals | NativeLessThan | NativeGreaterThan | Identical => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![None, None],
      }),
      Not => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("bool")))],
      }),

      // === String operations ===
      NativeStrConcat => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string"))), Some(Arc::new(Calcit::tag("string")))],
      }),
      Trim => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![None, Some(Arc::new(Calcit::tag("&")))],
      }),
      TurnString => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![None],
      }),
      NativeStr => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("&")))],
      }),
      Split => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string"))), Some(Arc::new(Calcit::tag("string")))],
      }),
      SplitLines => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string")))],
      }),
      StartsWith | EndsWith => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string"))), Some(Arc::new(Calcit::tag("string")))],
      }),
      GetCharCode => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string")))],
      }),
      CharFromCode => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("number")))],
      }),
      PrStr => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![None],
      }),
      ParseFloat => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string")))],
      }),
      IsBlank => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string")))],
      }),
      NativeStrCompare => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string"))), Some(Arc::new(Calcit::tag("string")))],
      }),
      NativeStrReplace => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![
          Some(Arc::new(Calcit::tag("string"))),
          Some(Arc::new(Calcit::tag("string"))),
          Some(Arc::new(Calcit::tag("string"))),
        ],
      }),
      NativeStrSlice => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![
          Some(Arc::new(Calcit::tag("string"))),
          Some(Arc::new(Calcit::tag("number"))),
          Some(Arc::new(Calcit::tag("&"))),
        ],
      }),
      NativeStrFindIndex => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string"))), Some(Arc::new(Calcit::tag("string")))],
      }),
      NativeStrEscape => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string")))],
      }),
      NativeStrCount => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string")))],
      }),
      NativeStrEmpty | NativeStrContains | NativeStrIncludes => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string")))],
      }),
      NativeStrNth | NativeStrFirst => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string")))],
      }),
      NativeStrRest => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string")))],
      }),
      NativeStrPadLeft | NativeStrPadRight => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![
          Some(Arc::new(Calcit::tag("string"))),
          Some(Arc::new(Calcit::tag("number"))),
          Some(Arc::new(Calcit::tag("string"))),
        ],
      }),

      // === List operations ===
      List => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("&")))],
      }),
      Append | Prepend => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("list"))), None],
      }),
      Butlast | NativeListReverse => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("list")))],
      }),
      Range => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("number"))), Some(Arc::new(Calcit::tag("&")))],
      }),
      Sort => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("list"))), Some(Arc::new(Calcit::tag("&")))],
      }),
      NativeListConcat => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("list"))), Some(Arc::new(Calcit::tag("&")))],
      }),
      NativeListCount => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("list")))],
      }),
      NativeListEmpty => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("list")))],
      }),
      NativeListContains | NativeListIncludes => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("list"))), None],
      }),
      NativeListSlice => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![
          Some(Arc::new(Calcit::tag("list"))),
          Some(Arc::new(Calcit::tag("number"))),
          Some(Arc::new(Calcit::tag("&"))),
        ],
      }),
      NativeListNth => Some(ProcTypeSignature {
        return_type: None,
        arg_types: vec![Some(Arc::new(Calcit::tag("list"))), Some(Arc::new(Calcit::tag("number")))],
      }),
      NativeListFirst => Some(ProcTypeSignature {
        return_type: None,
        arg_types: vec![Some(Arc::new(Calcit::tag("list")))],
      }),
      NativeListRest => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("list")))],
      }),
      NativeListAssoc | NativeListAssocBefore | NativeListAssocAfter => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("list"))), Some(Arc::new(Calcit::tag("number"))), None],
      }),
      NativeListDissoc => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("list"))), Some(Arc::new(Calcit::tag("number")))],
      }),
      NativeListToSet => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("set"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("list")))],
      }),
      NativeListDistinct => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("list")))],
      }),
      Foldl | FoldlShortcut | FoldrShortcut => Some(ProcTypeSignature {
        return_type: None,
        arg_types: vec![None, None, None],
      }),

      // === Map operations ===
      NativeMap => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("map"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("&")))],
      }),
      NativeMerge | NativeMergeNonNil => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("map"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("map"))), Some(Arc::new(Calcit::tag("map")))],
      }),
      ToPairs => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("set"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("map")))],
      }),
      NativeMapToList => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("map")))],
      }),
      NativeMapGet => Some(ProcTypeSignature {
        return_type: None,
        arg_types: vec![Some(Arc::new(Calcit::tag("map"))), None],
      }),
      NativeMapDissoc => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("map"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("map"))), None],
      }),
      NativeMapCount => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("map")))],
      }),
      NativeMapEmpty => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("map")))],
      }),
      NativeMapContains | NativeMapIncludes => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("map"))), None],
      }),
      NativeMapAssoc => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("map"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("map"))), None, None],
      }),
      NativeMapDiffNew | NativeMapDiffKeys | NativeMapCommonKeys => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("set"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("map"))), Some(Arc::new(Calcit::tag("map")))],
      }),

      // === Set operations ===
      Set => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("set"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("&")))],
      }),
      NativeInclude | NativeExclude => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("set"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("set"))), None],
      }),
      NativeDifference | NativeUnion | NativeSetIntersection => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("set"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("set"))), Some(Arc::new(Calcit::tag("set")))],
      }),
      NativeSetToList => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("set")))],
      }),
      NativeSetCount => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("set")))],
      }),
      NativeSetEmpty => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("set")))],
      }),
      NativeSetIncludes => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("set"))), None],
      }),

      // === Tuple operations ===
      NativeTuple => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("tuple"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("&")))],
      }),
      NativeTupleNth => Some(ProcTypeSignature {
        return_type: None,
        arg_types: vec![Some(Arc::new(Calcit::tag("tuple"))), Some(Arc::new(Calcit::tag("number")))],
      }),
      NativeTupleAssoc => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("tuple"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("tuple"))), Some(Arc::new(Calcit::tag("number"))), None],
      }),
      NativeTupleCount => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("tuple")))],
      }),
      NativeTupleClass => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("record"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("tuple")))],
      }),
      NativeTupleParams => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("tuple")))],
      }),

      // === Record operations ===
      NativeRecord => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("record"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("record"))), Some(Arc::new(Calcit::tag("&")))],
      }),
      NativeRecordWith => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("record"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("record"))), None, None, Some(Arc::new(Calcit::tag("&")))],
      }),
      NativeRecordAssoc => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("record"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("record"))), None, None],
      }),
      NativeRecordGet => Some(ProcTypeSignature {
        return_type: None,
        arg_types: vec![Some(Arc::new(Calcit::tag("record"))), Some(Arc::new(Calcit::tag("tag")))],
      }),
      NativeRecordCount => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("record")))],
      }),
      NativeRecordContains => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("record"))), None],
      }),
      NativeRecordMatches => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("bool"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("record"))), None],
      }),
      NativeRecordToMap => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("map"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("record")))],
      }),
      NativeRecordFromMap => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("record"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("record"))), Some(Arc::new(Calcit::tag("map")))],
      }),
      NativeRecordGetName => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("tag"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("record")))],
      }),
      NewRecord | NewClassRecord => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("record"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("tag"))), Some(Arc::new(Calcit::tag("&")))],
      }),
      NativeRecordClass => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("record"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("record")))],
      }),
      NativeRecordWithClass => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("record"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("record"))), None],
      }),
      NativeRecordExtendAs => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("record"))),
        arg_types: vec![
          Some(Arc::new(Calcit::tag("record"))),
          Some(Arc::new(Calcit::tag("tag"))),
          Some(Arc::new(Calcit::tag("&"))),
        ],
      }),

      // === Refs/Atoms ===
      Atom => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("ref"))),
        arg_types: vec![None],
      }),
      AtomDeref => Some(ProcTypeSignature {
        return_type: None,
        arg_types: vec![Some(Arc::new(Calcit::tag("ref")))],
      }),
      AddWatch => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("nil"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("ref"))), None, None],
      }),
      RemoveWatch => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("nil"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("ref"))), None],
      }),

      // === I/O operations ===
      ReadFile => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string")))],
      }),
      WriteFile => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("nil"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string"))), Some(Arc::new(Calcit::tag("string")))],
      }),
      GetEnv => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("string"))), Some(Arc::new(Calcit::tag("&")))],
      }),

      // === Time ===
      CpuTime => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("number"))),
        arg_types: vec![],
      }),

      // === Cirru format ===
      ParseCirru | ParseCirruEdn => Some(ProcTypeSignature {
        return_type: None,
        arg_types: vec![Some(Arc::new(Calcit::tag("string"))), Some(Arc::new(Calcit::tag("&")))],
      }),
      FormatCirru | FormatCirruEdn => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("string"))),
        arg_types: vec![None],
      }),
      ParseCirruList | NativeCirruQuoteToList => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("list"))),
        arg_types: vec![None],
      }),

      // === Buffer ===
      NativeBuffer => Some(ProcTypeSignature {
        return_type: Some(Arc::new(Calcit::tag("buffer"))),
        arg_types: vec![Some(Arc::new(Calcit::tag("&")))],
      }),

      // === Special forms and control flow ===
      // These typically don't have simple type signatures or are handled specially
      Recur
      | Raise
      | Quit
      | IsSpreadingMark
      | NativeResetGenSymIndex
      | NativeExtractCodeIntoEdn
      | NativeDataToCode
      | NativeCirruNth
      | NativeClassTuple
      | NativeEnumTuple
      | NativeTupleWithClass
      | NativeNumberDisplayBy
      | NativeMapDestruct
      | NativeSetDestruct
      | NativeFormatTernaryTree => None,
    }
  }

  /// Check if this proc has a type signature
  pub fn has_type_signature(&self) -> bool {
    self.get_type_signature().is_some()
  }
}
