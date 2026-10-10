//! Failure class of every builtin proc, shown by `calcit query def` / `query type`.
//!
//! Classes describe calls whose arguments already match the declared signature;
//! a value of the wrong type is a type error everywhere and is not repeated here.
//! The match below is exhaustive, so a new `CalcitProc` cannot compile without a class.

use super::CalcitProc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcFailure {
  /// Returns a value for every well-typed call.
  Total,
  /// Reports the expected failure through its return value (nil, Option or Result).
  Result(&'static str),
  /// Raises a catchable error (a WASM trap) under the stated condition.
  Raises(&'static str),
}

impl ProcFailure {
  /// Stable class name used in query output: `total`, `result` or `raises`.
  pub fn class_name(&self) -> &'static str {
    match self {
      Self::Total => "total",
      Self::Result(_) => "result",
      Self::Raises(_) => "raises",
    }
  }

  /// When the call reports or raises a failure; `None` for total procs.
  pub fn condition(&self) -> Option<&'static str> {
    match self {
      Self::Total => None,
      Self::Result(condition) | Self::Raises(condition) => Some(condition),
    }
  }
}

const INDEX: &str = "an index is not a non-negative integer or is out of range";
const CALLBACK: &str = "the callback raises";
const HOST_IO: &str = "the host file system operation fails";
const HOST_RESULT: &str = "host failures are returned as `Result :err`";

impl CalcitProc {
  pub fn failure(&self) -> ProcFailure {
    use CalcitProc::*;
    use ProcFailure::*;

    match self {
      // meta
      TypeOf
      | Recur
      | FormatToLisp
      | NativeResetGenSymIndex
      | NativeGetCalcitRunningMode
      | TurnSymbol
      | NativeCompare
      | NativeGetOs
      | NativeGetDefDoc
      | NativeHash
      | NativeCirruType
      | NativeDisplayStack
      | NativeMethodsOf
      | NativeInspectMethods
      | NativeInspectType
      | NativeGetCalcitBackend
      | RegisterCalcitBuiltinImpls
      | WithTypeSlot
      | IsSpreadingMark
      | NativeCirruQuoteToList => Total,
      FormatToCirru => Raises("the value is not a list of expressions"),
      GenerateId => Raises("the size is not a non-negative integer"),
      TurnTag => Raises("the value is not a string, tag or symbol"),
      NativeGetDefSchema => Raises("the definition is not found"),
      NativeFormatTernaryTree => Raises("the list is not stored as a ternary tree"),
      NativeBuffer => Raises("a byte is neither a number nor a two-digit hex string"),
      NativeExtractCodeIntoEdn => Raises("the value contains a ref, buffer, buf-list, recur or trait"),
      NativeDataToCode => Raises("the data contains a value with no code form, such as a function"),
      NativeCirruNth => Raises(INDEX),
      DeftypeSlot => Raises("the name is neither a tag nor a string"),

      // enums, structs, traits and impls as values
      NativeEnum
      | NativeEnumCount
      | NativeEnumImpls
      | NativeEnumParams
      | NativeEnumDefinition
      | NativeImplOrigin
      | NativeEnumDefHasVariant => Total,
      NativeNamedEnumNew => Raises("the variant is not defined or the payload does not match its arity"),
      NativeEnumNth | NativeEnumAssoc | NativeImplNth => Raises(INDEX),
      NativeStructNew | NativeEnumNew | NativeTraitNew | NativeImplNew => Raises("the definition data is malformed"),
      NativeStructValueImplTraits | NativeEnumValueImplTraits | NativeStructImplTraits | NativeEnumImplTraits => {
        Raises("an attached value is not an impl")
      }
      NativeImplGet => Raises("the impl has no such method"),
      NativeEnumDefVariantArity => Raises("the variant is not defined"),
      NativeEnumValidate => Raises("the value does not match its enum definition"),
      NativeTraitCall => Raises("the receiver has no impl for the trait method, or the method raises"),
      NativeAssertTraits => Raises("the value does not implement the traits"),

      // effects
      Raise | Todo => Raises("always"),
      Quit => Raises("the exit code is not an integer in 0..255; otherwise the process exits"),
      GetEnv => Result("nil when the variable is unset and no default is given"),
      GetArgs | MonotonicTimeMs => Total,
      UnixTimeMs => Raises("the system clock is before the Unix epoch"),
      NativeWaitMs | NativeSecureRandomBytes | NativeFsReadText | NativeReadStdinText | NativeFsReadDir | NativeFsWriteText => {
        Result(HOST_RESULT)
      }
      ReadFile | ReadDir | WriteFile => Raises(HOST_IO),

      // predicates
      ListQuestion | TagQuestion | SymbolQuestion | NilQuestion | StringQuestion | MapQuestion | NumberQuestion | BoolQuestion
      | SetQuestion | EnumQuestion | StructQuestion | FnQuestion | NativeListQ => Total,

      // external data formats
      ParseCirru | ParseCirruList => Raises("the text is not valid Cirru"),
      ParseCirruEdn => Raises("the text is not valid Cirru EDN"),
      JsonParse => Raises("the text is not valid JSON"),
      FormatCirru => Raises("the data is not a Cirru tree of strings and lists, or the inline option is not Bool"),
      FormatCirruOneLiner => Raises("the data is not a Cirru tree of strings and lists"),
      FormatCirruEdn => Raises("the value has no Cirru EDN form"),
      JsonStringify | JsonPretty => Raises("the value has no JSON form, including non-finite numbers"),

      // logics and math
      NativeEquals | NativeLessThan | NativeGreaterThan | Not | Identical => Total,
      NativeAdd | NativeMinus | NativeMultiply | NativeDivide | Round | Floor | Sin | Cos | Pow | Ceil | Sqrt | IsRound
      | NativeNumberFract => Total,
      NativeNumberRem => Raises("an operand is not a safe integer, or the divisor is zero"),
      NativeNumberFormat => Raises("the digit count is not an integer in 0..100"),
      NativeNumberDisplayBy => Raises("the value is not a non-negative integer, or the base is not 2, 8 or 16"),
      NativeNumberFits => Raises("the target is not a numeric refinement tag"),
      BitShl | BitShr | BitAnd | BitOr | BitXor | BitNot => Raises("an operand is not an integer"),

      // strings
      NativeStrConcat
      | Trim
      | NativeStr
      | Split
      | SplitLines
      | StartsWith
      | EndsWith
      | PrStr
      | IsBlank
      | NativeStrCompare
      | NativeStrReplace
      | NativeStrFindIndex
      | NativeStrEscape
      | NativeStrCount
      | NativeStrUtf8ByteCount
      | NativeStrEmpty
      | NativeStrIncludes
      | NativeStrFirst
      | NativeStrRest => Total,
      TurnString => Raises("the value is not a scalar"),
      GetCharCode => Raises("the string is not exactly one Unicode scalar"),
      CharFromCode => Raises("the code is not a Unicode scalar value"),
      ParseFloat => Result("nil when the text is not a number"),
      NativeStrSlice => Raises("a bound is not a non-negative integer"),
      NativeStrContains => Raises("the index is not a non-negative integer"),
      NativeStrNth => Result("nil when the index is out of range; raises when it is not a non-negative integer"),
      NativeStrPadLeft | NativeStrPadRight => Raises("the pattern is empty, or the length is past 536870888"),

      // lists
      List | Append | Prepend | Butlast | NativeListReverse | NativeListConcat | NativeListCount | NativeListEmpty
      | NativeListContains | NativeListIncludes | NativeListRest | NativeListToSet | NativeListDistinct | NativeListAppend
      | NativeListPrepend | NativeListButlast => Total,
      NativeListFirst | NativeListLast => Result("nil for an empty list"),
      Range | NativeListRange => {
        Raises("a bound or step is not finite, the step is zero or points away from the bound, or the result is too large")
      }
      Sort | NativeListSort => Raises("the comparator raises or returns a non-number"),
      Foldl | FoldlShortcut | FoldrShortcut | NativeListFoldl | NativeListFoldlShortcut => Raises(CALLBACK),
      NativeListSlice | NativeListAssocBefore | NativeListAssocAfter | NativeListNth | NativeListAssoc | NativeListDissoc => {
        Raises(INDEX)
      }

      // buf-list
      NativeBufListNew | NativeBufListPush | NativeBufListConcat | NativeBufListToList | NativeBufListCount => Total,

      // maps
      NativeMap | ToPairs | NativeMergeNonNil | NativeMapDissoc | NativeMapToList | NativeMapCount | NativeMapEmpty
      | NativeMapContains | NativeMapIncludes | NativeMapAssoc | NativeMapDiffNew | NativeMapDiffKeys | NativeMapCommonKeys
      | NativeMapDiffTriple | NativeMapKeys | NativeMapVals => Total,
      NativeMerge => Raises("merging into a struct names a field the struct does not have"),
      NativeMapGet => Result("nil for a missing key"),
      NativeMapDestruct => Result("nil for an empty map"),
      NativeMapFoldKv => Raises(CALLBACK),

      // sets
      Set
      | NativeInclude
      | NativeExclude
      | NativeDifference
      | NativeUnion
      | NativeSetIntersection
      | NativeSetToList
      | NativeSetCount
      | NativeSetEmpty
      | NativeSetIncludes => Total,
      NativeSetDestruct => Result("nil for an empty set"),

      // refs
      Ref | AtomDeref => Total,
      AddWatch => Raises("a listener with the same key already exists"),
      RemoveWatch => Raises("no listener has the key"),

      // structs
      NativeStructImpls
      | NativeStructMatches
      | NativeStructGetName
      | NativeStructDefinition
      | NativeStructToMap
      | NativeStructCount
      | NativeStructContains => Total,
      NativeLooseStruct => Raises("a field name is not a tag or appears twice"),
      NativeStruct => Raises("the fields do not match the struct definition"),
      NativeStructWith | NativeStructAssoc | NativeStructGet => Raises("the struct has no such field"),
      NativeStructFromMap => Raises("the map keys do not match the struct fields"),
      NativeStructNth | NativeStructFieldTag => Raises(INDEX),
      NativeStructAssocAt | NativeStructWithAt => Raises("the index is out of range or names a different field"),
      NativeStructExtendAs => Raises("an added field already exists"),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use strum::IntoEnumIterator;

  #[test]
  fn every_failing_proc_states_its_condition() {
    for proc in CalcitProc::iter() {
      let failure = proc.failure();
      match failure {
        ProcFailure::Total => assert!(failure.condition().is_none()),
        _ => assert!(
          failure.condition().is_some_and(|text| !text.trim().is_empty()),
          "{proc} needs a failure condition"
        ),
      }
    }
  }
}
