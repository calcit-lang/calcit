use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter, EnumString};

/// represent builtin functions for performance reasons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, EnumString, EnumIter, strum_macros::Display, AsRefStr)]
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
  #[strum(serialize = "&get-def-doc")]
  NativeGetDefDoc,
  #[strum(serialize = "&get-def-schema")]
  NativeGetDefSchema,
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
  NativeEnum,
  #[strum(serialize = "%::")]
  NativeNamedEnumNew,
  #[strum(serialize = "&enum:nth")]
  NativeEnumNth,
  #[strum(serialize = "&enum:assoc")]
  NativeEnumAssoc,
  #[strum(serialize = "&enum:count")]
  NativeEnumCount,
  #[strum(serialize = "&enum:impls")]
  NativeEnumImpls,
  #[strum(serialize = "&enum:params")]
  NativeEnumParams,
  #[strum(serialize = "&enum:definition")]
  NativeEnumDefinition,
  #[strum(serialize = "&struct-def:new")]
  NativeStructNew,
  #[strum(serialize = "&enum-def:new")]
  NativeEnumNew,
  #[strum(serialize = "&trait::new")]
  NativeTraitNew,
  #[strum(serialize = "&impl::new")]
  NativeImplNew,
  #[strum(serialize = "&struct:impl-traits")]
  NativeStructValueImplTraits,
  #[strum(serialize = "&enum:impl-traits")]
  NativeEnumValueImplTraits,
  #[strum(serialize = "&struct-def:impl-traits")]
  NativeStructImplTraits,
  #[strum(serialize = "&enum-def:impl-traits")]
  NativeEnumImplTraits,
  #[strum(serialize = "&impl:origin")]
  NativeImplOrigin,
  #[strum(serialize = "&impl:get")]
  NativeImplGet,
  #[strum(serialize = "&impl:nth")]
  NativeImplNth,
  #[strum(serialize = "&enum-def:has-variant?")]
  NativeEnumDefHasVariant,
  #[strum(serialize = "&enum-def:variant-arity")]
  NativeEnumDefVariantArity,
  #[strum(serialize = "&enum:validate")]
  NativeEnumValidate,
  #[strum(serialize = "&display-stack")]
  NativeDisplayStack,
  #[strum(serialize = "&methods-of")]
  NativeMethodsOf,
  #[strum(serialize = "&inspect-methods")]
  NativeInspectMethods,
  #[strum(serialize = "&trait-call")]
  NativeTraitCall,
  #[strum(serialize = "&inspect-type")]
  NativeInspectType,
  #[strum(serialize = "&assert-traits")]
  NativeAssertTraits,
  #[strum(serialize = "raise")]
  Raise,
  #[strum(serialize = "todo!")]
  Todo,
  #[strum(serialize = "quit!")]
  Quit,
  #[strum(serialize = "&get-env")]
  GetEnv,
  #[strum(serialize = "unix-time-ms")]
  UnixTimeMs,
  #[strum(serialize = "&get-calcit-backend")]
  NativeGetCalcitBackend,
  #[strum(serialize = "register-calcit-builtin-impls")]
  RegisterCalcitBuiltinImpls,
  #[strum(serialize = "read-file")]
  ReadFile,
  #[strum(serialize = "read-dir")]
  ReadDir,
  #[strum(serialize = "write-file")]
  WriteFile,
  #[strum(serialize = "list?")]
  ListQuestion,
  #[strum(serialize = "tag?")]
  TagQuestion,
  #[strum(serialize = "symbol?")]
  SymbolQuestion,
  #[strum(serialize = "nil?")]
  NilQuestion,
  #[strum(serialize = "string?")]
  StringQuestion,
  #[strum(serialize = "map?")]
  MapQuestion,
  #[strum(serialize = "number?")]
  NumberQuestion,
  #[strum(serialize = "bool?")]
  BoolQuestion,
  #[strum(serialize = "set?")]
  SetQuestion,
  #[strum(serialize = "enum?")]
  EnumQuestion,
  #[strum(serialize = "struct?")]
  StructQuestion,
  #[strum(serialize = "fn?")]
  FnQuestion,
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
  #[strum(serialize = "json-parse")]
  JsonParse,
  #[strum(serialize = "json-stringify")]
  JsonStringify,
  #[strum(serialize = "json-pretty")]
  JsonPretty,
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
  #[strum(serialize = "&parse-float")]
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
  #[strum(serialize = "&list:last")]
  NativeListLast,
  #[strum(serialize = "&list:append")]
  NativeListAppend,
  #[strum(serialize = "&list:prepend")]
  NativeListPrepend,
  #[strum(serialize = "&list:butlast")]
  NativeListButlast,
  #[strum(serialize = "&list:sort")]
  NativeListSort,
  #[strum(serialize = "&list:range")]
  NativeListRange,
  #[strum(serialize = "&list:foldl")]
  NativeListFoldl,
  #[strum(serialize = "&list:foldl-shortcut")]
  NativeListFoldlShortcut,
  // type predicate procs
  #[strum(serialize = "&list?")]
  NativeListQ,
  // buf-list (mutable append-only list)
  #[strum(serialize = "&buf-list:new")]
  NativeBufListNew,
  #[strum(serialize = "&buf-list:push")]
  NativeBufListPush,
  #[strum(serialize = "&buf-list:concat")]
  NativeBufListConcat,
  #[strum(serialize = "&buf-list:to-list")]
  NativeBufListToList,
  #[strum(serialize = "&buf-list:count")]
  NativeBufListCount,
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
  #[strum(serialize = "&map:diff-triple")]
  NativeMapDiffTriple,
  #[strum(serialize = "&map:keys")]
  NativeMapKeys,
  #[strum(serialize = "&map:vals")]
  NativeMapVals,
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
  #[strum(serialize = "?{}")]
  NativeLooseStruct,
  #[strum(serialize = "&%{}")]
  NativeStruct,
  #[strum(serialize = "&%{}?")]
  NativeStructPartial,
  #[strum(serialize = "&struct:with")]
  NativeStructWith,
  #[strum(serialize = "&struct:impls")]
  NativeStructImpls,
  #[strum(serialize = "&struct:matches?")]
  NativeStructMatches,
  #[strum(serialize = "&struct:from-map")]
  NativeStructFromMap,
  #[strum(serialize = "&struct:get-name")]
  NativeStructGetName,
  #[strum(serialize = "&struct:definition")]
  NativeStructDefinition,
  #[strum(serialize = "&struct:to-map")]
  NativeStructToMap,
  #[strum(serialize = "&struct:count")]
  NativeStructCount,
  #[strum(serialize = "&struct:contains?")]
  NativeStructContains,
  #[strum(serialize = "&struct:get")]
  NativeStructGet,
  #[strum(serialize = "&struct:nth")]
  NativeStructNth,
  #[strum(serialize = "&struct:field-tag")]
  NativeStructFieldTag,
  #[strum(serialize = "&struct:assoc")]
  NativeStructAssoc,
  #[strum(serialize = "&struct:assoc-at")]
  NativeStructAssocAt,
  #[strum(serialize = "&struct:with-at")]
  NativeStructWithAt,
  #[strum(serialize = "&struct:extend-as")]
  NativeStructExtendAs,
  // type slots
  #[strum(serialize = "deftype-slot")]
  DeftypeSlot,
  #[strum(serialize = "with-type-slot")]
  WithTypeSlot,
}

use crate::CalcitTypeAnnotation;

/// Type signature for a Proc (builtin function)
#[derive(Debug, Clone)]
pub struct ProcTypeSignature {
  /// return type declared
  pub return_type: Arc<CalcitTypeAnnotation>,
  /// Argument value types in order. Parameter omission is CalcitProc arity metadata;
  /// use Variadic to mark variadic args (no checking after this mark).
  pub arg_types: Vec<Arc<CalcitTypeAnnotation>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcArity {
  pub min: usize,
  pub max: Option<usize>,
}

impl ProcTypeSignature {
  fn arity(&self, optional_parameter_count: usize) -> ProcArity {
    let mut max = 0;
    let mut has_variadic = false;

    for t in &self.arg_types {
      match t.as_ref() {
        CalcitTypeAnnotation::Variadic(_) => {
          has_variadic = true;
          break;
        }
        _ => {
          max += 1;
        }
      }
    }

    debug_assert!(optional_parameter_count <= max);

    ProcArity {
      min: max.saturating_sub(optional_parameter_count),
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

fn set_of(inner: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Set(inner))
}

fn list_of(inner: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::List(inner))
}

fn map_of(key: Arc<CalcitTypeAnnotation>, value: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Map(key, value))
}

fn optional_tag(name: &str) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Optional(tag_type(name)))
}

fn optional_of(inner: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  match inner.as_ref() {
    CalcitTypeAnnotation::Optional(_) => inner,
    _ => Arc::new(CalcitTypeAnnotation::Optional(inner)),
  }
}

fn optional_dynamic() -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Optional(dynamic_tag()))
}

fn dynamic_tag() -> Arc<CalcitTypeAnnotation> {
  crate::calcit::type_annotation::DYNAMIC_TYPE.clone()
}

fn type_var(name: &str) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from(name)))
}

fn ref_of(inner: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Ref(inner))
}

fn variadic_of(inner: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Variadic(inner))
}

fn variadic_dynamic() -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Variadic(dynamic_tag()))
}

fn some_fn() -> Arc<CalcitTypeAnnotation> {
  tag_type("fn")
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
  pub fn get_type_signature(&self) -> Option<&'static ProcTypeSignature> {
    PROC_TYPE_SIGNATURES.get(self)
  }

  fn build_type_signature(&self) -> Option<ProcTypeSignature> {
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
        arg_types: vec![dynamic_tag()],
      }),
      NativeCompare => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![dynamic_tag(), dynamic_tag()],
      }),
      NativeGetOs => Some(ProcTypeSignature {
        return_type: some_tag("tag"),
        arg_types: vec![],
      }),
      NativeGetDefDoc => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![dynamic_tag()],
      }),
      NativeGetDefSchema => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag()],
      }),
      NativeHash => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![dynamic_tag()],
      }),
      GenerateId => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("number"), some_tag("string")],
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
      NativeMethodsOf => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![dynamic_tag()],
      }),
      NativeInspectMethods => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag(), some_tag("string")],
      }),
      NativeTraitCall => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("trait"), dynamic_tag(), dynamic_tag(), variadic_dynamic()],
      }),
      NativeAssertTraits => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![dynamic_tag(), some_tag("trait")],
      }),
      NativeCirruType => Some(ProcTypeSignature {
        return_type: some_tag("tag"),
        arg_types: vec![some_tag("cirru-quote")],
      }),
      NativeResetGenSymIndex => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![],
      }),
      NativeInspectType => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag(), some_tag("tag")],
      }),
      NativeExtractCodeIntoEdn => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag()],
      }),
      NativeDataToCode => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag()],
      }),
      ListQuestion => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),
      TagQuestion => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),
      SymbolQuestion => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),
      NilQuestion => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),
      StringQuestion => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),
      MapQuestion => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),
      NumberQuestion => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),
      BoolQuestion => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),
      SetQuestion => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),
      EnumQuestion => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),
      StructQuestion => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),
      FnQuestion => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
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
        arg_types: vec![dynamic_tag()],
      }),

      // === String operations ===
      NativeStrConcat => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![dynamic_tag(), dynamic_tag()],
      }),
      Trim => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("string"), some_tag("string")],
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
        arg_types: vec![some_tag("string"), some_tag("string")],
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
        return_type: optional_tag("number"),
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
        arg_types: vec![some_tag("string"), some_tag("number"), some_tag("number")],
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
      NativeStrContains => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("string"), some_tag("number")],
      }),
      NativeStrIncludes => Some(ProcTypeSignature {
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
        return_type: list_of(type_var("T")),
        arg_types: vec![variadic_of(type_var("T"))],
      }),
      Append | Prepend | NativeListAppend | NativeListPrepend => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("list"), dynamic_tag()],
      }),
      Butlast | NativeListReverse | NativeListButlast => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("list")],
      }),
      Range | NativeListRange => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("number"), some_tag("number"), some_tag("number")],
      }),
      Sort | NativeListSort => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("list"), some_fn()],
      }),
      NativeListConcat => Some(ProcTypeSignature {
        return_type: list_of(type_var("T")),
        arg_types: vec![variadic_of(list_of(type_var("T")))],
      }),
      NativeListQ => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),
      NativeListCount => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![list_of(type_var("T"))],
      }),
      NativeListEmpty => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![list_of(type_var("T"))],
      }),
      NativeListContains => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![list_of(type_var("T")), some_tag("number")],
      }),
      NativeListIncludes => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![list_of(type_var("T")), type_var("T")],
      }),
      NativeListSlice => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("list"), some_tag("number"), some_tag("number")],
      }),
      NativeListNth => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![list_of(type_var("T")), some_tag("number")],
      }),
      NativeListFirst => Some(ProcTypeSignature {
        return_type: optional_of(type_var("T")),
        arg_types: vec![list_of(type_var("T"))],
      }),
      NativeListRest => Some(ProcTypeSignature {
        return_type: list_of(type_var("T")),
        arg_types: vec![list_of(type_var("T"))],
      }),
      NativeListAssoc | NativeListAssocBefore | NativeListAssocAfter => Some(ProcTypeSignature {
        return_type: list_of(type_var("T")),
        arg_types: vec![list_of(type_var("T")), some_tag("number"), type_var("T")],
      }),
      NativeListDissoc => Some(ProcTypeSignature {
        return_type: list_of(type_var("T")),
        arg_types: vec![list_of(type_var("T")), some_tag("number")],
      }),
      NativeListToSet => Some(ProcTypeSignature {
        return_type: set_of(type_var("T")),
        arg_types: vec![list_of(type_var("T"))],
      }),
      NativeListDistinct => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("list")],
      }),
      NativeListLast => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("list")],
      }),
      // === BufList operations ===
      NativeBufListNew => Some(ProcTypeSignature {
        return_type: some_tag("buf-list"),
        arg_types: vec![],
      }),
      NativeBufListPush => Some(ProcTypeSignature {
        return_type: some_tag("buf-list"),
        arg_types: vec![some_tag("buf-list"), type_var("T")],
      }),
      NativeBufListConcat => Some(ProcTypeSignature {
        return_type: some_tag("buf-list"),
        arg_types: vec![some_tag("buf-list"), list_of(type_var("T"))],
      }),
      NativeBufListToList => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("buf-list")],
      }),
      NativeBufListCount => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("buf-list")],
      }),
      Foldl => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag(), dynamic_tag(), some_fn()],
      }),
      FoldlShortcut | FoldrShortcut | NativeListFoldlShortcut => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag(), dynamic_tag(), dynamic_tag(), some_fn()],
      }),
      NativeListFoldl => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag(), dynamic_tag(), some_fn()],
      }),

      // === Map operations ===
      NativeMap => Some(ProcTypeSignature {
        return_type: some_tag("map"),
        arg_types: vec![variadic_dynamic()],
      }),
      NativeMerge => Some(ProcTypeSignature {
        return_type: map_of(type_var("K"), type_var("V")),
        arg_types: vec![
          map_of(type_var("K"), type_var("V")),
          map_of(type_var("K"), type_var("V")),
          variadic_of(map_of(type_var("K"), type_var("V"))),
        ],
      }),
      NativeMergeNonNil => Some(ProcTypeSignature {
        return_type: map_of(type_var("K"), type_var("V")),
        arg_types: vec![map_of(type_var("K"), type_var("V")), map_of(type_var("K"), type_var("V"))],
      }),
      ToPairs => Some(ProcTypeSignature {
        return_type: some_set(),
        arg_types: vec![some_tag("map")],
      }),
      NativeMapToList => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![map_of(type_var("K"), type_var("V"))],
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
        arg_types: vec![map_of(type_var("K"), type_var("V"))],
      }),
      NativeMapEmpty => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![map_of(type_var("K"), type_var("V"))],
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
        return_type: map_of(type_var("K"), type_var("W")),
        arg_types: vec![map_of(type_var("K"), type_var("V")), map_of(type_var("K"), type_var("W"))],
      }),
      NativeMapDiffKeys | NativeMapCommonKeys => Some(ProcTypeSignature {
        return_type: set_of(type_var("K")),
        arg_types: vec![map_of(type_var("K"), type_var("V")), map_of(type_var("K"), type_var("W"))],
      }),
      NativeMapDiffTriple => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![map_of(type_var("K"), type_var("V")), map_of(type_var("K"), type_var("W"))],
      }),
      NativeMapKeys => Some(ProcTypeSignature {
        return_type: set_of(type_var("K")),
        arg_types: vec![map_of(type_var("K"), type_var("V"))],
      }),
      NativeMapVals => Some(ProcTypeSignature {
        return_type: set_of(type_var("V")),
        arg_types: vec![map_of(type_var("K"), type_var("V"))],
      }),
      NativeMapDestruct => Some(ProcTypeSignature {
        return_type: optional_tag("list"),
        arg_types: vec![map_of(type_var("K"), type_var("V"))],
      }),

      // === Set operations ===
      Set => Some(ProcTypeSignature {
        return_type: set_of(type_var("T")),
        arg_types: vec![variadic_of(type_var("T"))],
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
        arg_types: vec![set_of(type_var("T"))],
      }),
      NativeSetEmpty => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![set_of(type_var("T"))],
      }),
      NativeSetIncludes => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_set(), dynamic_tag()],
      }),
      NativeSetDestruct => Some(ProcTypeSignature {
        return_type: optional_tag("list"),
        arg_types: vec![set_of(type_var("T"))],
      }),

      // === Enum value operations ===
      NativeEnum => Some(ProcTypeSignature {
        return_type: some_tag("enum"),
        arg_types: vec![dynamic_tag(), variadic_dynamic()],
      }),
      NativeNamedEnumNew => Some(ProcTypeSignature {
        return_type: some_tag("enum"),
        arg_types: vec![some_tag("enum-def"), some_tag("tag"), variadic_dynamic()],
      }),
      NativeEnumNth => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("enum"), some_tag("number")],
      }),
      NativeEnumAssoc => Some(ProcTypeSignature {
        return_type: some_tag("enum"),
        arg_types: vec![some_tag("enum"), some_tag("number"), dynamic_tag()],
      }),
      NativeEnumCount => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("enum")],
      }),
      NativeEnumImpls => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("enum")],
      }),
      NativeEnumParams => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("enum")],
      }),
      NativeEnumDefinition => Some(ProcTypeSignature {
        return_type: optional_tag("enum-def"),
        arg_types: vec![some_tag("enum")],
      }),
      NativeStructNew => Some(ProcTypeSignature {
        return_type: some_tag("struct-def"),
        arg_types: vec![some_tag("tag"), variadic_dynamic()],
      }),
      NativeEnumNew => Some(ProcTypeSignature {
        return_type: some_tag("enum-def"),
        arg_types: vec![some_tag("tag"), variadic_dynamic()],
      }),
      NativeTraitNew => Some(ProcTypeSignature {
        return_type: some_tag("trait"),
        arg_types: vec![dynamic_tag(), some_tag("list")],
      }),
      NativeImplNew => Some(ProcTypeSignature {
        return_type: some_tag("impl"),
        arg_types: vec![dynamic_tag(), variadic_dynamic()],
      }),
      NativeImplOrigin => Some(ProcTypeSignature {
        return_type: optional_tag("trait"),
        arg_types: vec![some_tag("impl")],
      }),
      NativeImplGet => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("impl"), dynamic_tag()],
      }),
      NativeImplNth => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("impl"), some_tag("number")],
      }),
      NativeStructValueImplTraits => Some(ProcTypeSignature {
        return_type: some_tag("struct"),
        arg_types: vec![some_tag("struct"), variadic_dynamic()],
      }),
      NativeEnumValueImplTraits => Some(ProcTypeSignature {
        return_type: some_tag("enum"),
        arg_types: vec![some_tag("enum"), variadic_dynamic()],
      }),
      NativeStructImplTraits => Some(ProcTypeSignature {
        return_type: some_tag("struct-def"),
        arg_types: vec![some_tag("struct-def"), variadic_dynamic()],
      }),
      NativeEnumImplTraits => Some(ProcTypeSignature {
        return_type: some_tag("enum-def"),
        arg_types: vec![some_tag("enum-def"), variadic_dynamic()],
      }),
      NativeEnumDefHasVariant => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("enum-def"), some_tag("tag")],
      }),
      NativeEnumDefVariantArity => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("enum-def"), some_tag("tag")],
      }),
      NativeEnumValidate => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![some_tag("enum"), some_tag("tag")],
      }),

      // === Struct value operations ===
      NativeLooseStruct => Some(ProcTypeSignature {
        return_type: some_tag("struct"),
        arg_types: vec![variadic_dynamic()],
      }),
      NativeStruct => Some(ProcTypeSignature {
        return_type: some_tag("struct"),
        arg_types: vec![some_tag("struct-def"), variadic_dynamic()],
      }),
      NativeStructPartial => Some(ProcTypeSignature {
        return_type: some_tag("struct"),
        arg_types: vec![some_tag("struct-def"), variadic_dynamic()],
      }),
      NativeStructWith => Some(ProcTypeSignature {
        return_type: some_tag("struct"),
        arg_types: vec![some_tag("struct"), dynamic_tag(), dynamic_tag(), variadic_dynamic()],
      }),
      NativeStructAssoc => Some(ProcTypeSignature {
        return_type: some_tag("struct"),
        arg_types: vec![some_tag("struct"), dynamic_tag(), dynamic_tag()],
      }),
      NativeStructAssocAt => Some(ProcTypeSignature {
        return_type: some_tag("struct"),
        arg_types: vec![some_tag("struct"), some_tag("number"), some_tag("tag"), dynamic_tag()],
      }),
      NativeStructWithAt => Some(ProcTypeSignature {
        return_type: some_tag("struct"),
        // (struct, idx, tag, value, ...) — variadic triples after first arg
        arg_types: vec![
          some_tag("struct"),
          some_tag("number"),
          some_tag("tag"),
          dynamic_tag(),
          variadic_dynamic(),
        ],
      }),
      NativeStructGet => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("struct"), some_tag("tag")],
      }),
      NativeStructNth => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("struct"), some_tag("number"), some_tag("tag")],
      }),
      NativeStructFieldTag => Some(ProcTypeSignature {
        return_type: some_tag("tag"),
        arg_types: vec![some_tag("struct"), some_tag("number")],
      }),
      NativeStructCount => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![some_tag("struct")],
      }),
      NativeStructContains => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("struct"), dynamic_tag()],
      }),
      NativeStructMatches => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![some_tag("struct"), dynamic_tag()],
      }),
      NativeStructToMap => Some(ProcTypeSignature {
        return_type: map_of(some_tag("tag"), dynamic_tag()),
        arg_types: vec![some_tag("struct")],
      }),
      NativeStructFromMap => Some(ProcTypeSignature {
        return_type: some_tag("struct"),
        arg_types: vec![some_tag("struct-def"), some_tag("map")],
      }),
      NativeStructGetName => Some(ProcTypeSignature {
        return_type: some_tag("tag"),
        arg_types: vec![some_tag("struct")],
      }),
      NativeStructDefinition => Some(ProcTypeSignature {
        return_type: optional_tag("struct-def"),
        arg_types: vec![some_tag("struct")],
      }),
      NativeStructImpls => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("struct")],
      }),
      NativeStructExtendAs => Some(ProcTypeSignature {
        return_type: some_tag("struct"),
        arg_types: vec![some_tag("struct"), some_tag("tag"), some_tag("tag"), dynamic_tag()],
      }),

      // === Refs/Atoms ===
      Atom => Some(ProcTypeSignature {
        return_type: some_tag("ref"),
        arg_types: vec![dynamic_tag()],
      }),
      AtomDeref => Some(ProcTypeSignature {
        return_type: type_var("T"),
        arg_types: vec![ref_of(type_var("T"))],
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
      ReadDir => Some(ProcTypeSignature {
        return_type: list_of(some_tag("string")),
        arg_types: vec![some_tag("string"), some_tag("bool")],
      }),
      WriteFile => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![some_tag("string"), some_tag("string")],
      }),
      Raise => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![variadic_dynamic()],
      }),
      Todo => Some(ProcTypeSignature {
        // `todo!` is a diverging compiler-known placeholder. Dynamic keeps it
        // assignable to any declared return type while the static W_TODO
        // diagnostic prevents it from being silently forgotten.
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("string")],
      }),
      Quit => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![some_tag("number")],
      }),
      GetEnv => Some(ProcTypeSignature {
        return_type: optional_dynamic(),
        arg_types: vec![some_tag("string"), dynamic_tag()],
      }),
      UnixTimeMs => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![],
      }),

      // === Time ===
      CpuTime => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![],
      }),

      // === Cirru format ===
      ParseCirru => Some(ProcTypeSignature {
        return_type: some_tag("cirru-quote"),
        arg_types: vec![some_tag("string")],
      }),
      ParseCirruEdn => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("string"), dynamic_tag()],
      }),
      JsonParse => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("string")],
      }),
      JsonStringify | JsonPretty => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![dynamic_tag()],
      }),
      FormatCirru | FormatCirruEdn => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![dynamic_tag(), some_tag("bool")],
      }),
      FormatCirruOneLiner => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![dynamic_tag()],
      }),
      ParseCirruList => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![dynamic_tag()],
      }),
      NativeCirruQuoteToList => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("cirru-quote")],
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

      // === Type slot operations ===
      DeftypeSlot => Some(ProcTypeSignature {
        return_type: some_tag("nil"),
        arg_types: vec![some_tag("tag")],
      }),
      // with-type-slot has a variadic body; type-checking is handled at preprocess time
      WithTypeSlot => None,
    }
  }

  /// Check if this proc has a type signature
  pub fn has_type_signature(&self) -> bool {
    self.get_type_signature().is_some()
  }

  /// Return the runtime call arity without encoding parameter omission as a nullable value type.
  pub fn arity(&self) -> Option<ProcArity> {
    self
      .get_type_signature()
      .map(|signature| signature.arity(self.optional_parameter_count()))
  }

  fn optional_parameter_count(&self) -> usize {
    use CalcitProc::*;
    match self {
      GenerateId | Range | NativeListRange => 2,
      NativeInspectMethods | NativeInspectType | Trim | NativeStrSlice | Sort | NativeListSort | NativeListSlice | NativeStructNth
      | ReadDir | GetEnv | ParseCirruEdn | FormatCirru | FormatCirruEdn | Todo => 1,
      _ => 0,
    }
  }
}

static PROC_TYPE_SIGNATURES: LazyLock<HashMap<CalcitProc, ProcTypeSignature>> = LazyLock::new(|| {
  CalcitProc::iter()
    .filter_map(|proc| proc.build_type_signature().map(|signature| (proc, signature)))
    .collect()
});

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use super::*;

  #[test]
  fn optional_proc_parameters_do_not_make_values_nullable() {
    let trim = CalcitProc::Trim.get_type_signature().expect("trim signature");
    assert_eq!(CalcitProc::Trim.arity(), Some(ProcArity { min: 1, max: Some(2) }));
    assert!(matches!(trim.arg_types[1].as_ref(), CalcitTypeAnnotation::String));

    let generate_id = CalcitProc::GenerateId.get_type_signature().expect("generate-id signature");
    assert_eq!(CalcitProc::GenerateId.arity(), Some(ProcArity { min: 0, max: Some(2) }));
    assert!(matches!(generate_id.arg_types[0].as_ref(), CalcitTypeAnnotation::Number));

    for proc in [
      CalcitProc::GenerateId,
      CalcitProc::Range,
      CalcitProc::NativeListRange,
      CalcitProc::NativeInspectMethods,
      CalcitProc::NativeInspectType,
      CalcitProc::Trim,
      CalcitProc::NativeStrSlice,
      CalcitProc::Sort,
      CalcitProc::NativeListSort,
      CalcitProc::NativeListSlice,
      CalcitProc::NativeStructNth,
      CalcitProc::ReadDir,
      CalcitProc::GetEnv,
      CalcitProc::ParseCirruEdn,
      CalcitProc::FormatCirru,
      CalcitProc::FormatCirruEdn,
    ] {
      let signature = proc.get_type_signature().expect("migrated proc signature");
      assert!(
        signature
          .arg_types
          .iter()
          .all(|arg| !matches!(arg.as_ref(), CalcitTypeAnnotation::Optional(_))),
        "{proc} must not encode parameter omission as Optional<T>"
      );
    }

    let explicitly_nullable = ProcTypeSignature {
      return_type: dynamic_tag(),
      arg_types: vec![optional_tag("string")],
    };
    assert_eq!(explicitly_nullable.arity(0), ProcArity { min: 1, max: Some(1) });
    assert!(matches!(
      explicitly_nullable.arg_types[0].as_ref(),
      CalcitTypeAnnotation::Optional(inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::String)
    ));

    for (proc, expected) in [
      (CalcitProc::GenerateId, ProcArity { min: 0, max: Some(2) }),
      (CalcitProc::Todo, ProcArity { min: 0, max: Some(1) }),
      (CalcitProc::Range, ProcArity { min: 1, max: Some(3) }),
      (CalcitProc::NativeListRange, ProcArity { min: 1, max: Some(3) }),
      (CalcitProc::NativeInspectMethods, ProcArity { min: 1, max: Some(2) }),
      (CalcitProc::NativeInspectType, ProcArity { min: 1, max: Some(2) }),
      (CalcitProc::Trim, ProcArity { min: 1, max: Some(2) }),
      (CalcitProc::NativeStrSlice, ProcArity { min: 2, max: Some(3) }),
      (CalcitProc::Sort, ProcArity { min: 1, max: Some(2) }),
      (CalcitProc::NativeListSort, ProcArity { min: 1, max: Some(2) }),
      (CalcitProc::NativeListSlice, ProcArity { min: 2, max: Some(3) }),
      (CalcitProc::NativeStructNth, ProcArity { min: 2, max: Some(3) }),
      (CalcitProc::ReadDir, ProcArity { min: 1, max: Some(2) }),
      (CalcitProc::GetEnv, ProcArity { min: 1, max: Some(2) }),
      (CalcitProc::ParseCirruEdn, ProcArity { min: 1, max: Some(2) }),
      (CalcitProc::FormatCirru, ProcArity { min: 1, max: Some(2) }),
      (CalcitProc::FormatCirruEdn, ProcArity { min: 1, max: Some(2) }),
    ] {
      assert_eq!(proc.arity(), Some(expected), "{proc} arity");
    }
  }

  #[test]
  fn absence_returning_proc_signatures_are_nullable() {
    let parse_float = CalcitProc::ParseFloat.get_type_signature().expect("parse-float signature");
    assert!(matches!(
      parse_float.return_type.as_ref(),
      CalcitTypeAnnotation::Optional(inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::Number)
    ));

    let get_env = CalcitProc::GetEnv.get_type_signature().expect("get-env signature");
    assert!(matches!(
      get_env.return_type.as_ref(),
      CalcitTypeAnnotation::Optional(inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
    ));

    let record_struct = CalcitProc::NativeStructDefinition
      .get_type_signature()
      .expect("record-struct signature");
    assert_eq!(record_struct.return_type, optional_tag("struct-def"));

    let list_first = CalcitProc::NativeListFirst.get_type_signature().expect("&list:first signature");
    assert!(matches!(
      list_first.return_type.as_ref(),
      CalcitTypeAnnotation::Optional(inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T")
    ));
  }

  #[test]
  fn nullable_primitives_do_not_shadow_their_typed_public_wrappers() {
    assert_eq!(CalcitProc::from_str("&parse-float"), Ok(CalcitProc::ParseFloat));
    assert_eq!(CalcitProc::from_str("&get-env"), Ok(CalcitProc::GetEnv));
    assert_eq!(CalcitProc::from_str("&struct:nth"), Ok(CalcitProc::NativeStructNth));
    assert_eq!(CalcitProc::from_str("&enum:nth"), Ok(CalcitProc::NativeEnumNth));
    assert!(CalcitProc::from_str("parse-float").is_err());
    assert!(CalcitProc::from_str("get-env").is_err());
  }
}
