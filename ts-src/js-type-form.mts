import { CalcitValue } from "./js-primes.mjs";
import { CalcitSymbol, CalcitTag } from "./calcit-data.mjs";
import { CalcitList, CalcitSliceList } from "./js-list.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";
import { CalcitSet } from "./js-set.mjs";
import { CalcitRef } from "./js-ref.mjs";
import { CalcitEnumValue } from "./js-enum-value.mjs";
import { CalcitCirruQuote } from "./js-cirru.mjs";

/// Shallow runtime check of a value against a struct field type form, matching
/// the native `value_matches_type_annotation`: container kinds are checked,
/// members are not. Names that cannot be resolved here (aliases, traits, type
/// variables) stay permissive only for the legacy, statically checked path.
/// Dynamic writes require evidence: deep contracts use generated validators,
/// while this fallback accepts only completely checkable scalar/open forms.
export let valueMatchesTypeForm = (value: CalcitValue, form: CalcitValue, requireEvidence = false): boolean =>
  matchTypeForm(value, form, requireEvidence) === true;

// null means that the contract cannot be checked here, not a mismatched value.
// Preserve that distinction through Optional/JsNullish instead of letting an
// absent payload hide an unresolved contract.
let matchTypeForm = (value: CalcitValue, form: CalcitValue, requireEvidence: boolean): boolean | null => {
  if (form instanceof CalcitSliceList || form instanceof CalcitList) {
    const items = (form as CalcitList | CalcitSliceList).toArray();
    const head = items[0];
    if (head instanceof CalcitSymbol && head.value === "quote") {
      return matchTypeForm(value, items[1], requireEvidence);
    }
    if (head instanceof CalcitSymbol && head.value === "::") {
      const name = typeFormName(items[1]);
      // Nil is `null` here; `undefined` is Unit. Only the JS host boundary
      // type also admits `undefined`.
      if (name === "Optional") {
        const inner = matchTypeForm(value, items[2], requireEvidence);
        return inner == null ? null : value === null || inner;
      }
      if (name === "JsNullish") {
        const inner = matchTypeForm(value, items[2], requireEvidence);
        return inner == null ? null : value == null || inner;
      }
      if (name === "Fn" && requireEvidence) return null;
      return name == null ? (requireEvidence ? null : true) : valueMatchesTypeName(value, name, requireEvidence);
    }
    return requireEvidence ? null : true;
  }
  const name = typeFormName(form);
  return name == null ? (requireEvidence ? null : true) : valueMatchesTypeName(value, name, requireEvidence);
};

let typeFormName = (form: CalcitValue): string | null => {
  if (form instanceof CalcitSymbol) return form.value;
  if (form instanceof CalcitTag) return form.value;
  if (form instanceof CalcitSliceList || form instanceof CalcitList) {
    const items = (form as CalcitList | CalcitSliceList).toArray();
    if (items[0] instanceof CalcitSymbol && items[0].value === "quote") return typeFormName(items[1]);
  }
  return null;
};

/// Numeric refinements check range and integrality like the native `CalcitNumericRefinement::accepts`.
let isIntegerIn = (value: CalcitValue, min: number, max: number): boolean =>
  typeof value === "number" && Number.isInteger(value) && value >= min && value <= max;

let isList = (value: CalcitValue) => value instanceof CalcitList || value instanceof CalcitSliceList;
let isMap = (value: CalcitValue) => value instanceof CalcitMap || value instanceof CalcitSliceMap;

let valueMatchesTypeName = (value: CalcitValue, rawName: string, requireEvidence: boolean): boolean | null => {
  const name = rawName.startsWith("calcit.core/") ? rawName.slice("calcit.core/".length) : rawName;
  switch (name) {
    case "Dynamic":
    case "dynamic":
    case "any":
    case "JsObject":
      return true;
    case "Number":
    case "number":
    case "Float64":
    case "float64":
      return typeof value === "number";
    case "Float32":
    case "float32":
      return typeof value === "number" && Math.fround(value) === value;
    case "Int8":
    case "int8":
      return isIntegerIn(value, -128, 127);
    case "UInt8":
    case "uint8":
      return isIntegerIn(value, 0, 255);
    case "Int16":
    case "int16":
      return isIntegerIn(value, -32768, 32767);
    case "UInt16":
    case "uint16":
      return isIntegerIn(value, 0, 65535);
    case "Int32":
    case "int32":
      return isIntegerIn(value, -2147483648, 2147483647);
    case "UInt32":
    case "uint32":
      return isIntegerIn(value, 0, 4294967295);
    case "Int64":
    case "int64":
      return isIntegerIn(value, -Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER);
    case "UInt64":
    case "uint64":
      return isIntegerIn(value, 0, Number.MAX_SAFE_INTEGER);
    case "String":
    case "string":
      return typeof value === "string";
    case "Bool":
    case "bool":
      return typeof value === "boolean";
    case "Tag":
    case "tag":
      return value instanceof CalcitTag;
    case "Symbol":
    case "symbol":
      return value instanceof CalcitSymbol;
    case "Nil":
    case "nil":
      return value === null;
    case "Unit":
    case "unit":
      return value === undefined;
    case "Never":
      return false;
    case "Buffer":
    case "buffer":
      return value instanceof Uint8Array;
    case "CirruQuote":
    case "cirru-quote":
      return value instanceof CalcitCirruQuote;
    case "List":
    case "list":
      return requireEvidence ? null : isList(value);
    case "Map":
    case "map":
      return requireEvidence ? null : isMap(value);
    case "Set":
    case "set":
      return requireEvidence ? null : value instanceof CalcitSet;
    case "Ref":
    case "ref":
      return requireEvidence ? null : value instanceof CalcitRef;
    case "Fn":
    case "fn":
      return typeof value === "function";
    case "Struct":
    case "struct":
    case "record":
      return requireEvidence ? null : isStructValue(value);
    case "Enum":
    case "enum":
    case "tuple":
      return requireEvidence ? null : value instanceof CalcitEnumValue;
  }
  // Deep/nominal contracts must use the compiler's resolved validator. A
  // short-name match or an erased generic cannot establish runtime evidence.
  if (requireEvidence) return null;
  // Core Struct and Enum definitions (bare names resolve to them through the
  // implicit core import) are known here, so a reference to one admits only
  // values of that definition, as the native matcher does.
  if (CORE_NOMINAL_DEFS.has(name)) {
    if (isStructValue(value)) return (value as any).structRef.definitionRef === `calcit.core/${name}`;
    return value instanceof CalcitEnumValue && value.enumPrototype?.prototype.structRef.definitionRef === `calcit.core/${name}`;
  }
  // Other names may be type variables, aliases or traits that cannot be
  // resolved here. A qualified reference is checked against the value's own
  // Struct or Enum name; other values are left to the static checker like
  // unresolved forms.
  const shortName = name.includes("/") ? name.slice(name.lastIndexOf("/") + 1) : name;
  if (isStructValue(value)) return (value as any).name.value === shortName || !name.includes("/");
  if (value instanceof CalcitEnumValue && value.enumPrototype != null) {
    return value.enumPrototype.name() === shortName || !name.includes("/");
  }
  return true;
};

/// Struct and Enum definitions of `calcit.core` usable as field types.
const CORE_NOMINAL_DEFS = new Set([
  "Option",
  "Result",
  "Data",
  "ListDestruct",
  "MapDestruct",
  "SetDestruct",
  "StringDestruct",
  "MapEntry",
  "RuntimeMapMeta",
  "RuntimeMapResponse",
]);

// Duck-typed to avoid a module cycle with js-struct-value.
let isStructValue = (value: CalcitValue): boolean =>
  value != null && typeof value === "object" && "structRef" in (value as object) && "fields" in (value as object);
