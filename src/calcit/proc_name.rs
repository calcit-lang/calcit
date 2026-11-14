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
  #[strum(serialize = "&union-def:new")]
  NativeUnionNew,
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
  #[strum(serialize = "unreachable!")]
  Unreachable,
  /// Lowered `unreachable!`: one String message that already names the site.
  #[strum(serialize = "&unreachable!")]
  NativeUnreachable,
  #[strum(serialize = "quit!")]
  Quit,
  #[strum(serialize = "&get-env")]
  GetEnv,
  #[strum(serialize = "&get-args")]
  GetArgs,
  #[strum(serialize = "unix-time-ms")]
  UnixTimeMs,
  #[strum(serialize = "&wait-ms")]
  NativeWaitMs,
  #[strum(serialize = "&secure-random-bytes")]
  NativeSecureRandomBytes,
  #[strum(serialize = "&get-calcit-backend")]
  NativeGetCalcitBackend,
  #[strum(serialize = "register-calcit-builtin-impls")]
  RegisterCalcitBuiltinImpls,
  #[strum(serialize = "&read-file")]
  ReadFile,
  #[strum(serialize = "&fs-read-text")]
  NativeFsReadText,
  #[strum(serialize = "&read-stdin-text")]
  NativeReadStdinText,
  #[strum(serialize = "&fs-read-dir")]
  NativeFsReadDir,
  #[strum(serialize = "&read-dir")]
  ReadDir,
  #[strum(serialize = "write-file")]
  WriteFile,
  #[strum(serialize = "&fs-write-text")]
  NativeFsWriteText,
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
  #[strum(serialize = "&cirru-quote:leaf-text")]
  NativeCirruQuoteLeafText,
  // time
  #[strum(serialize = "monotonic-time-ms")]
  MonotonicTimeMs,
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
  #[strum(serialize = "integer?")]
  IsInteger,
  #[strum(serialize = "&number:fract")]
  NativeNumberFract,
  #[strum(serialize = "&number:rem")]
  NativeNumberRem,
  #[strum(serialize = "&number:format")]
  NativeNumberFormat,
  #[strum(serialize = "&number:display-by")]
  NativeNumberDisplayBy,
  #[strum(serialize = "&number:fits?")]
  NativeNumberFits,
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
  #[strum(serialize = "&turn-string")]
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
  #[strum(serialize = "&str:utf8-byte-count")]
  NativeStrUtf8ByteCount,
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
  #[strum(serialize = "&map:fold-kv")]
  NativeMapFoldKv,
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
  /// `ref` constructs a local `Ref<T>`.
  #[strum(serialize = "ref")]
  Ref,
  #[strum(serialize = "&atom:deref")]
  AtomDeref,
  #[strum(serialize = "add-watch!")]
  AddWatch,
  #[strum(serialize = "remove-watch!")]
  RemoveWatch,
  // records
  #[strum(serialize = "?{}")]
  NativeLooseStruct,
  #[strum(serialize = "&%{}")]
  NativeStruct,
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
  /// Argument value types in order. Parameter omission is CalcitProc arity metadata.
  /// `Variadic<T>` supplies rest-element inference evidence; procs whose runtime
  /// contract requires every rest argument to match `T` opt into checking it.
  pub arg_types: Vec<Arc<CalcitTypeAnnotation>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

fn set_of(inner: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Set(inner))
}

fn list_of(inner: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::List(inner))
}

fn map_of(key: Arc<CalcitTypeAnnotation>, value: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Map(key, value))
}

/// Nominal `calcit.core/Option<T>` (#1554): absence-returning primitives return `Option :none`, never nil.
fn option_of(inner: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::TypeRef(
    Arc::from("calcit.core/Option"),
    Arc::new(vec![inner]),
  ))
}

fn option_tag(name: &str) -> Arc<CalcitTypeAnnotation> {
  option_of(tag_type(name))
}

fn dynamic_tag() -> Arc<CalcitTypeAnnotation> {
  crate::calcit::type_annotation::DYNAMIC_TYPE.clone()
}

fn type_var(name: &str) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from(name)))
}

/// A callback the proc calls before it returns, marked `(:features :pass)`:
/// it accepts callbacks with any effect, charged to the caller (#2726).
fn pass_callback_slot(args: Vec<Arc<CalcitTypeAnnotation>>, ret: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  fn_slot_with_features(args, ret, &[crate::calcit::PASS_EFFECTS_FEATURE])
}

fn fn_slot_with_features(
  args: Vec<Arc<CalcitTypeAnnotation>>,
  ret: Arc<CalcitTypeAnnotation>,
  features: &[&str],
) -> Arc<CalcitTypeAnnotation> {
  let CalcitTypeAnnotation::Fn(signature) = CalcitTypeAnnotation::from_function_parts(args, ret) else {
    unreachable!("function parts produce a function signature")
  };
  let mut signature = signature.as_ref().clone();
  signature.features = Arc::new(features.iter().map(|feature| cirru_edn::EdnTag::from(*feature)).collect());
  Arc::new(CalcitTypeAnnotation::Fn(Arc::new(signature)))
}

/// A Ref watcher, `Fn T T (-> Unit) (:features :state :io)`. It is stored and
/// runs later when the Ref changes, so it is not a `:pass` slot; it may read
/// other Refs or print (#2726).
fn watcher_slot() -> Arc<CalcitTypeAnnotation> {
  fn_slot_with_features(vec![type_var("T"), type_var("T")], some_tag("unit"), &["state", "io"])
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
  /// Whether a typed `Variadic<T>` is a homogeneous call contract rather than
  /// collection-literal inference evidence.
  pub(crate) fn checks_variadic_arg_types(&self) -> bool {
    matches!(
      self,
      Self::NativeListConcat
        | Self::NativeMerge
        | Self::NativeMapDissoc
        | Self::NativeStructValueImplTraits
        | Self::NativeEnumValueImplTraits
        | Self::NativeStructImplTraits
        | Self::NativeEnumImplTraits
    )
  }

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

  fn signature_from_core(&self, core: &crate::snapshot::FileInSnapShot) -> ProcTypeSignature {
    let name = self.as_ref();
    let Some(CalcitTypeAnnotation::Fn(schema)) = core.defs.get(name).map(|entry| entry.schema.as_ref()) else {
      panic!("bundled proc {name} requires a function schema");
    };
    // Type variables and nested callback effects are carried by the argument
    // and return types. ProcTypeSignature cannot carry outer bounds or effects.
    assert!(
      schema.fn_kind == super::type_annotation::SchemaKind::Fn && schema.where_bounds.is_empty() && schema.features.is_empty(),
      "bundled proc {name} requires a function schema without outer bounds or effects"
    );

    let qualify = |annotation: &Arc<CalcitTypeAnnotation>| {
      super::type_refs::map_type_references(annotation.clone(), &|name, args| {
        let name = if core.defs.contains_key(name.as_ref()) {
          Arc::from(format!("{}/{name}", super::CORE_NS))
        } else {
          name.clone()
        };
        Arc::new(CalcitTypeAnnotation::TypeRef(name, args))
      })
    };
    let mut arg_types: Vec<_> = schema.arg_types.iter().map(&qualify).collect();
    if let Some(rest) = &schema.rest_type {
      arg_types.push(variadic_of(qualify(rest)));
    }
    ProcTypeSignature {
      arg_types,
      return_type: qualify(&schema.return_type),
    }
  }

  fn build_type_signature(&self, core: &crate::snapshot::FileInSnapShot) -> Option<ProcTypeSignature> {
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
        return_type: some_tag("unit"),
        arg_types: vec![variadic_dynamic()],
      }),
      // Method values are callable (native `Calcit::Method`, JS invoke
      // closures), so the honest element type is a function (#2534).
      NativeMethodsOf => Some(ProcTypeSignature {
        return_type: list_of(Arc::new(CalcitTypeAnnotation::DynFn)),
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
        return_type: type_var("T"),
        arg_types: vec![type_var("T"), some_tag("trait")],
      }),
      NativeCirruType => Some(ProcTypeSignature {
        return_type: some_tag("tag"),
        arg_types: vec![some_tag("cirru-quote")],
      }),
      NativeResetGenSymIndex => Some(ProcTypeSignature {
        return_type: some_tag("unit"),
        arg_types: vec![],
      }),
      NativeInspectType => Some(ProcTypeSignature {
        return_type: dynamic_tag(),
        arg_types: vec![dynamic_tag(), some_tag("tag")],
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

      // Runtime intrinsics whose argument and return types come from core.
      NativeAdd
      | NativeMinus
      | NativeMultiply
      | NativeDivide
      | Pow
      | NativeNumberRem
      | Floor
      | Ceil
      | Round
      | Sin
      | Cos
      | Sqrt
      | NativeNumberFract
      | IsInteger
      | BitShl
      | BitShr
      | BitAnd
      | BitOr
      | BitXor
      | BitNot
      | NativeNumberFormat
      | NativeNumberDisplayBy
      | NativeNumberFits
      | NativeExtractCodeIntoEdn
      | NativeDataToCode
      | NativeListQ
      | NativeListReverse
      | NativeListButlast
      | NativeListRange
      | NativeListSort
      | NativeListConcat
      | NativeListCount
      | NativeListEmpty
      | NativeListContains
      | NativeListIncludes
      | NativeListSlice
      | NativeListNth
      | NativeListFirst
      | NativeListRest
      | NativeListAssoc
      | NativeListAssocBefore
      | NativeListAssocAfter
      | NativeListDissoc
      | NativeListToSet
      | NativeListDistinct
      | NativeListLast
      | NativeMerge
      | NativeMergeNonNil
      | ToPairs
      | NativeMapToList
      | NativeMapFoldKv
      | NativeMapDissoc
      | NativeMapCount
      | NativeMapEmpty
      | NativeMapContains
      | NativeMapIncludes
      | NativeMapDiffNew
      | NativeMapDiffKeys
      | NativeMapCommonKeys
      | NativeMapDiffTriple
      | NativeMapKeys
      | NativeMapVals
      | NativeMapDestruct
      | NativeSetToList
      | NativeSetCount
      | NativeSetEmpty
      | NativeSetIncludes
      | NativeSetDestruct
      | NativeStrConcat
      | TurnString
      | Split
      | SplitLines
      | StartsWith
      | EndsWith
      | GetCharCode
      | CharFromCode
      | ParseFloat
      | IsBlank
      | NativeStrCompare
      | NativeStrReplace
      | NativeStrSlice
      | NativeStrFindIndex
      | NativeStrEscape
      | NativeStrCount
      | NativeStrUtf8ByteCount
      | NativeStrEmpty
      | NativeStrContains
      | NativeStrIncludes
      | NativeStrNth
      | NativeStrFirst
      | NativeStrRest
      | NativeStrPadLeft
      | NativeStrPadRight
      | NativeLessThan
      | NativeGreaterThan => Some(self.signature_from_core(core)),

      // === Comparison & Logic ===
      NativeEquals | Identical => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag(), dynamic_tag()],
      }),
      Not => Some(ProcTypeSignature {
        return_type: some_tag("bool"),
        arg_types: vec![dynamic_tag()],
      }),

      // === String operations ===
      Trim => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("string"), some_tag("string")],
      }),
      NativeStr => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![variadic_dynamic()],
      }),
      PrStr => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![dynamic_tag()],
      }),

      // === List operations ===
      List => Some(ProcTypeSignature {
        return_type: list_of(type_var("T")),
        arg_types: vec![variadic_of(type_var("T"))],
      }),
      Append | Prepend | NativeListAppend | NativeListPrepend => Some(ProcTypeSignature {
        return_type: list_of(type_var("T")),
        arg_types: vec![list_of(type_var("T")), type_var("T")],
      }),
      Butlast => Some(ProcTypeSignature {
        return_type: list_of(type_var("T")),
        arg_types: vec![list_of(type_var("T"))],
      }),
      Range => Some(ProcTypeSignature {
        return_type: list_of(some_tag("number")),
        arg_types: vec![some_tag("number"), some_tag("number"), some_tag("number")],
      }),
      Sort => {
        let item_type = type_var("T");
        Some(ProcTypeSignature {
          return_type: list_of(item_type.clone()),
          arg_types: vec![
            list_of(item_type.clone()),
            pass_callback_slot(vec![item_type.clone(), item_type], tag_type("number")),
          ],
        })
      }
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
      FoldrShortcut | NativeListFoldlShortcut => Some(ProcTypeSignature {
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
      NativeMapGet => Some(ProcTypeSignature {
        return_type: option_of(type_var("V")),
        arg_types: vec![map_of(type_var("K"), type_var("V")), type_var("K")],
      }),
      NativeMapAssoc => Some(ProcTypeSignature {
        return_type: map_of(type_var("K"), type_var("V")),
        arg_types: vec![
          map_of(type_var("K"), type_var("V")),
          type_var("K"),
          type_var("V"),
          variadic_dynamic(),
        ],
      }),

      // === Set operations ===
      Set => Some(ProcTypeSignature {
        return_type: set_of(type_var("T")),
        arg_types: vec![variadic_of(type_var("T"))],
      }),
      NativeInclude => Some(ProcTypeSignature {
        return_type: set_of(type_var("T")),
        arg_types: vec![set_of(type_var("T")), type_var("T")],
      }),
      NativeExclude => Some(ProcTypeSignature {
        return_type: set_of(type_var("T")),
        arg_types: vec![set_of(type_var("T")), type_var("U")],
      }),
      NativeDifference | NativeSetIntersection => Some(ProcTypeSignature {
        return_type: set_of(type_var("T")),
        arg_types: vec![set_of(type_var("T")), set_of(type_var("U"))],
      }),
      NativeUnion => Some(ProcTypeSignature {
        return_type: set_of(type_var("T")),
        arg_types: vec![set_of(type_var("T")), set_of(type_var("T"))],
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
        return_type: list_of(some_tag("impl")),
        arg_types: vec![some_tag("enum")],
      }),
      NativeEnumParams => Some(ProcTypeSignature {
        return_type: some_tag("list"),
        arg_types: vec![some_tag("enum")],
      }),
      NativeEnumDefinition => Some(ProcTypeSignature {
        return_type: option_tag("enum-def"),
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
      NativeUnionNew => {
        let schema = core.defs[self.as_ref()]
          .schema
          .as_function()
          .expect("bundled UnionDef constructor schema");
        let mut arg_types = schema.arg_types.clone();
        arg_types.push(variadic_of(schema.rest_type.clone().expect("UnionDef constructor rest parameter")));
        Some(ProcTypeSignature {
          return_type: schema.return_type.clone(),
          arg_types,
        })
      }
      NativeTraitNew => Some(ProcTypeSignature {
        return_type: some_tag("trait"),
        arg_types: vec![dynamic_tag(), some_tag("list"), some_tag("list")],
      }),
      NativeImplNew => Some(ProcTypeSignature {
        return_type: some_tag("impl"),
        arg_types: vec![dynamic_tag(), variadic_dynamic()],
      }),
      NativeImplOrigin => Some(ProcTypeSignature {
        return_type: option_tag("trait"),
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
        arg_types: vec![some_tag("struct"), some_tag("impl"), variadic_of(some_tag("impl"))],
      }),
      NativeEnumValueImplTraits => Some(ProcTypeSignature {
        return_type: some_tag("enum"),
        arg_types: vec![some_tag("enum"), some_tag("impl"), variadic_of(some_tag("impl"))],
      }),
      NativeStructImplTraits => Some(ProcTypeSignature {
        return_type: some_tag("struct-def"),
        arg_types: vec![some_tag("struct-def"), some_tag("impl"), variadic_of(some_tag("impl"))],
      }),
      NativeEnumImplTraits => Some(ProcTypeSignature {
        return_type: some_tag("enum-def"),
        arg_types: vec![some_tag("enum-def"), some_tag("impl"), variadic_of(some_tag("impl"))],
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
        return_type: some_tag("unit"),
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
      NativeStructWith | NativeStructAssoc | NativeStructContains | NativeStructMatches | NativeStructFromMap
      | NativeStructExtendAs => {
        let schema = core.defs[self.as_ref()]
          .schema
          .as_function()
          .expect("bundled Struct primitive schema");
        let mut arg_types = schema.arg_types.clone();
        arg_types.extend(schema.rest_type.iter().cloned().map(variadic_of));
        Some(ProcTypeSignature {
          return_type: schema.return_type.clone(),
          arg_types,
        })
      }
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
      NativeStructToMap => Some(ProcTypeSignature {
        return_type: map_of(some_tag("tag"), dynamic_tag()),
        arg_types: vec![some_tag("struct")],
      }),
      NativeStructGetName => Some(ProcTypeSignature {
        return_type: some_tag("tag"),
        arg_types: vec![some_tag("struct")],
      }),
      NativeStructDefinition => Some(ProcTypeSignature {
        return_type: option_tag("struct-def"),
        arg_types: vec![some_tag("struct")],
      }),
      NativeStructImpls => Some(ProcTypeSignature {
        return_type: list_of(some_tag("impl")),
        arg_types: vec![some_tag("struct")],
      }),

      // === Refs/Atoms ===
      Ref => Some(ProcTypeSignature {
        return_type: ref_of(type_var("T")),
        arg_types: vec![type_var("T")],
      }),
      AtomDeref => Some(ProcTypeSignature {
        return_type: type_var("T"),
        arg_types: vec![ref_of(type_var("T"))],
      }),
      AddWatch => Some(ProcTypeSignature {
        return_type: some_tag("unit"),
        arg_types: vec![ref_of(type_var("T")), some_tag("tag"), watcher_slot()],
      }),
      RemoveWatch => Some(ProcTypeSignature {
        return_type: some_tag("unit"),
        arg_types: vec![ref_of(type_var("T")), some_tag("tag")],
      }),

      // === I/O operations ===
      ReadFile | ReadDir => {
        let schema = core.defs[self.as_ref()]
          .schema
          .as_function()
          .expect("bundled file read function schema");
        Some(ProcTypeSignature {
          return_type: schema.return_type.clone(),
          arg_types: schema.arg_types.clone(),
        })
      }
      NativeFsReadText => Some(ProcTypeSignature {
        return_type: Arc::new(CalcitTypeAnnotation::TypeRef(
          Arc::from("Result"),
          Arc::new(vec![some_tag("string"), some_tag("string")]),
        )),
        arg_types: vec![some_tag("enum-def"), some_tag("string"), some_tag("string")],
      }),
      NativeReadStdinText => Some(ProcTypeSignature {
        return_type: Arc::new(CalcitTypeAnnotation::TypeRef(
          Arc::from("Result"),
          Arc::new(vec![some_tag("string"), some_tag("string")]),
        )),
        arg_types: vec![some_tag("enum-def"), some_tag("string")],
      }),
      NativeFsReadDir => {
        // Concrete result types come from the constructors passed at the call site.
        let schema = core.defs[self.as_ref()]
          .schema
          .as_function()
          .expect("bundled directory read function schema");
        Some(ProcTypeSignature {
          return_type: schema.return_type.clone(),
          arg_types: schema.arg_types.clone(),
        })
      }
      WriteFile => Some(ProcTypeSignature {
        return_type: some_tag("unit"),
        arg_types: vec![some_tag("string"), some_tag("string")],
      }),
      NativeFsWriteText => Some(ProcTypeSignature {
        return_type: Arc::new(CalcitTypeAnnotation::TypeRef(
          Arc::from("Result"),
          Arc::new(vec![some_tag("unit"), some_tag("string")]),
        )),
        arg_types: vec![some_tag("enum-def"), some_tag("string"), some_tag("string"), some_tag("string")],
      }),
      Raise => Some(ProcTypeSignature {
        return_type: crate::calcit::type_annotation::NEVER_TYPE.clone(),
        arg_types: vec![variadic_dynamic()],
      }),
      Todo => Some(ProcTypeSignature {
        // `todo!` is a diverging compiler-known placeholder. Dynamic keeps it
        // assignable to any declared return type while the static E_TODO
        // diagnostic prevents it from being silently forgotten.
        return_type: dynamic_tag(),
        arg_types: vec![some_tag("string")],
      }),
      Unreachable | NativeUnreachable => {
        let schema = core.defs[self.as_ref()]
          .schema
          .as_function()
          .expect("bundled unreachable function schema");
        Some(ProcTypeSignature {
          return_type: schema.return_type.clone(),
          arg_types: schema.arg_types.clone(),
        })
      }
      Quit => Some(ProcTypeSignature {
        return_type: some_tag("unit"),
        arg_types: vec![some_tag("number")],
      }),
      GetEnv => Some(ProcTypeSignature {
        return_type: option_tag("string"),
        arg_types: vec![some_tag("string")],
      }),
      GetArgs => Some(ProcTypeSignature {
        return_type: list_of(some_tag("string")),
        arg_types: vec![],
      }),
      UnixTimeMs => Some(ProcTypeSignature {
        return_type: some_tag("number"),
        arg_types: vec![],
      }),
      NativeWaitMs => Some(ProcTypeSignature {
        return_type: Arc::new(CalcitTypeAnnotation::TypeRef(
          Arc::from("Result"),
          Arc::new(vec![some_tag("unit"), some_tag("string")]),
        )),
        arg_types: vec![some_tag("enum-def"), some_tag("number"), some_tag("string")],
      }),
      NativeSecureRandomBytes => Some(ProcTypeSignature {
        return_type: Arc::new(CalcitTypeAnnotation::TypeRef(
          Arc::from("Result"),
          Arc::new(vec![some_tag("buffer"), some_tag("string")]),
        )),
        arg_types: vec![some_tag("enum-def"), some_tag("number"), some_tag("string")],
      }),

      // === Time ===
      MonotonicTimeMs => Some(ProcTypeSignature {
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
      FormatCirru => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("list"), some_tag("bool")],
      }),
      FormatCirruEdn => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![dynamic_tag(), some_tag("bool")],
      }),
      FormatCirruOneLiner => Some(ProcTypeSignature {
        return_type: some_tag("string"),
        arg_types: vec![some_tag("list")],
      }),
      ParseCirruList => Some(ProcTypeSignature {
        return_type: list_of(cirru_node()),
        arg_types: vec![some_tag("string")],
      }),
      NativeCirruQuoteToList => Some(ProcTypeSignature {
        return_type: list_of(cirru_node()),
        arg_types: vec![some_tag("cirru-quote")],
      }),
      NativeCirruQuoteLeafText => Some(ProcTypeSignature {
        return_type: some_tag("string"),
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
      RegisterCalcitBuiltinImpls => Some(ProcTypeSignature {
        return_type: some_tag("unit"),
        arg_types: vec![dynamic_tag()],
      }),

      // === Special forms and control flow ===
      // These typically don't have simple type signatures or are handled specially
      Recur => None,

      // === Type slot operations ===
      DeftypeSlot => Some(ProcTypeSignature {
        return_type: some_tag("unit"),
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

  /// Primitives that report absence as `Option :none` and so construct values of the
  /// nominal `calcit.core/Option` at runtime. Backends that only emit reachable
  /// definitions must keep that definition (#1554).
  pub fn returns_core_option(&self) -> bool {
    self
      .get_type_signature()
      .is_some_and(|signature| signature.return_type.is_option_type())
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
      | ReadDir | ParseCirruEdn | FormatCirru | FormatCirruEdn | Todo | Unreachable | NativeTraitNew => 1,
      _ => 0,
    }
  }
}

static PROC_TYPE_SIGNATURES: LazyLock<HashMap<CalcitProc, ProcTypeSignature>> = LazyLock::new(|| {
  let snapshot = crate::load_core_snapshot().expect("load bundled core proc signatures");
  let core = &snapshot.files[super::CORE_NS];
  CalcitProc::iter()
    .filter_map(|proc| proc.build_type_signature(core).map(|signature| (proc, signature)))
    .collect()
});

/// `calcit.core/CirruNode`, the recursive `String | List CirruNode` union.
pub(crate) fn cirru_node() -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("calcit.core/CirruNode"), Arc::new(vec![])))
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use super::*;

  #[test]
  fn variadic_proc_schemas_match_runtime_signatures() {
    let snapshot = crate::load_core_snapshot().expect("bundled core snapshot");
    let definitions = &snapshot.files[crate::calcit::CORE_NS].defs;
    for proc in [CalcitProc::NativeBuffer, CalcitProc::NativeDisplayStack, CalcitProc::NativeMapAssoc] {
      let core = definitions[proc.as_ref()].schema.as_function().expect("core function schema");
      let runtime = proc.get_type_signature().expect("runtime function signature");
      let mut args = core.arg_types.clone();
      let rest = core.rest_type.as_ref().expect("variadic schema has a rest parameter");
      args.push(variadic_of(rest.clone()));
      assert_eq!(args, runtime.arg_types, "{proc} argument types");
      assert_eq!(core.return_type, runtime.return_type, "{proc} return type");
    }
  }

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
      CalcitProc::ParseCirruEdn,
      CalcitProc::FormatCirru,
      CalcitProc::FormatCirruEdn,
    ] {
      let signature = proc.get_type_signature().expect("migrated proc signature");
      assert!(
        signature.arg_types.iter().all(|arg| !matches!(
          arg.as_ref(),
          CalcitTypeAnnotation::Nullable(_, crate::calcit::NullableOrigin::Inferred)
        )),
        "{proc} must not encode parameter omission as Optional<T>"
      );
    }

    let explicitly_nullable = ProcTypeSignature {
      return_type: dynamic_tag(),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::Nullable(
        tag_type("string"),
        crate::calcit::NullableOrigin::Inferred,
      ))],
    };
    assert_eq!(explicitly_nullable.arity(0), ProcArity { min: 1, max: Some(1) });
    assert!(matches!(
      explicitly_nullable.arg_types[0].as_ref(),
      CalcitTypeAnnotation::Nullable(inner, crate::calcit::NullableOrigin::Inferred) if matches!(inner.as_ref(), CalcitTypeAnnotation::String)
    ));

    for (proc, expected) in [
      (CalcitProc::GenerateId, ProcArity { min: 0, max: Some(2) }),
      (CalcitProc::Todo, ProcArity { min: 0, max: Some(1) }),
      (CalcitProc::Unreachable, ProcArity { min: 0, max: Some(1) }),
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
      (CalcitProc::ParseCirruEdn, ProcArity { min: 1, max: Some(2) }),
      (CalcitProc::FormatCirru, ProcArity { min: 1, max: Some(2) }),
      (CalcitProc::FormatCirruEdn, ProcArity { min: 1, max: Some(2) }),
    ] {
      assert_eq!(proc.arity(), Some(expected), "{proc} arity");
    }
  }

  #[test]
  fn collection_and_ref_primitives_preserve_generic_payloads() {
    let nth = CalcitProc::NativeListNth.get_type_signature().expect("list nth signature");
    assert!(matches!(
      nth.return_type.as_ref(),
      CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T"
    ));
    assert!(matches!(
      nth.arg_types.first().map(AsRef::as_ref),
      Some(CalcitTypeAnnotation::List(inner))
        if matches!(inner.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T")
    ));

    let map_contains = CalcitProc::NativeMapContains.get_type_signature().expect("map contains signature");
    assert!(matches!(
      map_contains.arg_types.get(1).map(AsRef::as_ref),
      Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "K"
    ));
    let map_includes = CalcitProc::NativeMapIncludes.get_type_signature().expect("map includes signature");
    assert!(matches!(
      map_includes.arg_types.get(1).map(AsRef::as_ref),
      Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "V"
    ));
    let map_assoc = CalcitProc::NativeMapAssoc.get_type_signature().expect("map assoc signature");
    assert!(matches!(
      map_assoc.return_type.as_ref(),
      CalcitTypeAnnotation::Map(key, value)
        if matches!(key.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "K")
          && matches!(value.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "V")
    ));
    assert!(matches!(
      map_assoc.arg_types.get(1).map(AsRef::as_ref),
      Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "K"
    ));
    assert!(matches!(
      map_assoc.arg_types.get(2).map(AsRef::as_ref),
      Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "V"
    ));
    let map_dissoc = CalcitProc::NativeMapDissoc.get_type_signature().expect("map dissoc signature");
    assert!(matches!(
      map_dissoc.return_type.as_ref(),
      CalcitTypeAnnotation::Map(key, value)
        if matches!(key.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "K")
          && matches!(value.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "V")
    ));
    assert!(matches!(
      map_dissoc.arg_types.get(2).map(AsRef::as_ref),
      Some(CalcitTypeAnnotation::Variadic(inner))
        if matches!(inner.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "K")
    ));
    let set_includes = CalcitProc::NativeSetIncludes.get_type_signature().expect("set includes signature");
    assert!(matches!(
      set_includes.arg_types.first().map(AsRef::as_ref),
      Some(CalcitTypeAnnotation::Set(inner))
        if matches!(inner.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T")
    ));
    assert!(matches!(
      set_includes.arg_types.get(1).map(AsRef::as_ref),
      Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "T"
    ));

    for proc in [CalcitProc::Sort, CalcitProc::NativeListSort] {
      let sort = proc.get_type_signature().expect("sort signature");
      assert!(matches!(
        sort.return_type.as_ref(),
        CalcitTypeAnnotation::List(inner)
          if matches!(inner.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T")
      ));
      assert!(matches!(
        sort.arg_types.get(1).map(AsRef::as_ref),
        Some(CalcitTypeAnnotation::Fn(comparator))
          if matches!(comparator.arg_types.as_slice(), [left, right]
            if matches!(left.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T")
              && matches!(right.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T"))
            && matches!(comparator.return_type.as_ref(), CalcitTypeAnnotation::Number)
      ));
    }

    // The removed `atom` spelling is no longer a proc name.
    assert!(CalcitProc::from_str("atom").is_err());
    assert_eq!(CalcitProc::from_str("ref"), Ok(CalcitProc::Ref));
    assert_eq!(CalcitProc::Ref.as_ref(), "ref");
    let atom = CalcitProc::Ref.get_type_signature().expect("ref signature");
    assert!(matches!(
      atom.return_type.as_ref(),
      CalcitTypeAnnotation::Ref(inner)
        if matches!(inner.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T")
    ));
    assert!(matches!(
      atom.arg_types.first().map(AsRef::as_ref),
      Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "T"
    ));

    let add_watch = CalcitProc::AddWatch.get_type_signature().expect("add-watch signature");
    assert!(matches!(
      add_watch.arg_types.as_slice(),
      [reference, key, callback]
        if matches!(reference.as_ref(), CalcitTypeAnnotation::Ref(inner)
          if matches!(inner.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T"))
          && matches!(key.as_ref(), CalcitTypeAnnotation::Tag)
          && matches!(callback.as_ref(), CalcitTypeAnnotation::Fn(signature)
            if matches!(signature.arg_types.as_slice(), [current, previous]
              if matches!(current.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T")
                && matches!(previous.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T"))
              && matches!(signature.return_type.as_ref(), CalcitTypeAnnotation::Unit))
    ));

    let remove_watch = CalcitProc::RemoveWatch.get_type_signature().expect("remove-watch! signature");
    assert!(matches!(
      remove_watch.arg_types.as_slice(),
      [reference, key]
        if matches!(reference.as_ref(), CalcitTypeAnnotation::Ref(inner)
          if matches!(inner.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T"))
          && matches!(key.as_ref(), CalcitTypeAnnotation::Tag)
    ));
  }

  #[test]
  fn typed_variadic_proc_policy_distinguishes_contracts_from_literal_inference() {
    for proc in [CalcitProc::NativeListConcat, CalcitProc::NativeMerge, CalcitProc::NativeMapDissoc] {
      assert!(proc.checks_variadic_arg_types(), "{proc} should enforce every typed rest argument");
    }
    for proc in [CalcitProc::List, CalcitProc::Set] {
      assert!(
        !proc.checks_variadic_arg_types(),
        "{proc} should infer a common literal item type and preserve heterogeneous fallback"
      );
    }
  }

  #[test]
  fn absence_returning_proc_signatures_return_nominal_option() {
    for proc in [
      CalcitProc::ParseFloat,
      CalcitProc::GetEnv,
      CalcitProc::NativeStrFirst,
      CalcitProc::NativeStrNth,
      CalcitProc::NativeListFirst,
      CalcitProc::NativeListLast,
      CalcitProc::NativeMapGet,
      CalcitProc::NativeEnumDefinition,
      CalcitProc::NativeImplOrigin,
      CalcitProc::NativeStructDefinition,
    ] {
      let signature = proc.get_type_signature().expect("absence-returning proc signature");
      assert!(
        signature.return_type.is_option_type(),
        "{proc} must return nominal Option<T>, got {:?}",
        signature.return_type
      );
    }

    let parse_float = CalcitProc::ParseFloat.get_type_signature().expect("parse-float signature");
    assert_eq!(parse_float.return_type, option_tag("number"));
    let struct_definition = CalcitProc::NativeStructDefinition
      .get_type_signature()
      .expect("struct-definition signature");
    assert_eq!(struct_definition.return_type, option_tag("struct-def"));

    for (proc, name, parameters) in [
      (
        CalcitProc::NativeMapDestruct,
        "calcit.core/MapDestruct",
        vec![type_var("K"), type_var("V")],
      ),
      (CalcitProc::NativeSetDestruct, "calcit.core/SetDestruct", vec![type_var("T")]),
    ] {
      assert_eq!(
        proc.get_type_signature().unwrap().return_type,
        Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from(name), Arc::new(parameters)))
      );
    }

    let list_first = CalcitProc::NativeListFirst.get_type_signature().expect("&list:first signature");
    assert!(
      matches!(list_first.return_type.option_payload().as_deref(), Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "T")
    );
    assert_eq!(CalcitProc::GetEnv.arity(), Some(ProcArity { min: 1, max: Some(1) }));
  }

  #[test]
  fn option_primitives_do_not_shadow_their_typed_public_wrappers() {
    assert_eq!(CalcitProc::from_str("&parse-float"), Ok(CalcitProc::ParseFloat));
    assert_eq!(CalcitProc::from_str("&get-env"), Ok(CalcitProc::GetEnv));
    assert_eq!(CalcitProc::from_str("&struct:nth"), Ok(CalcitProc::NativeStructNth));
    assert_eq!(CalcitProc::from_str("&enum:nth"), Ok(CalcitProc::NativeEnumNth));
    assert!(CalcitProc::from_str("parse-float").is_err());
    assert!(CalcitProc::from_str("get-env").is_err());
    assert_eq!(CalcitProc::from_str("&read-file"), Ok(CalcitProc::ReadFile));
    assert_eq!(CalcitProc::from_str("&read-dir"), Ok(CalcitProc::ReadDir));
    assert!(CalcitProc::from_str("read-file").is_err());
    assert!(CalcitProc::from_str("read-dir").is_err());
  }

  #[test]
  fn effect_proc_signatures_use_unit_and_assertions_preserve_values() {
    for proc in [
      CalcitProc::NativeDisplayStack,
      CalcitProc::NativeResetGenSymIndex,
      CalcitProc::NativeEnumValidate,
      CalcitProc::AddWatch,
      CalcitProc::RemoveWatch,
      CalcitProc::WriteFile,
      CalcitProc::Quit,
      CalcitProc::RegisterCalcitBuiltinImpls,
      CalcitProc::DeftypeSlot,
    ] {
      let signature = proc.get_type_signature().expect("effect proc signature");
      assert!(matches!(signature.return_type.as_ref(), CalcitTypeAnnotation::Unit), "{proc}");
    }

    let signature = CalcitProc::NativeAssertTraits
      .get_type_signature()
      .expect("assert-traits signature");
    assert_eq!(signature.return_type, signature.arg_types[0]);
    assert!(matches!(
      signature.return_type.as_ref(),
      CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T"
    ));
  }
}
