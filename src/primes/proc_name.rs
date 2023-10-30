use strum_macros::EnumString;

/// represent builtin functions for performance reasons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, EnumString, strum_macros::Display)]
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
