import { CalcitValue } from "./js-primes.mjs";
import { CalcitSymbol, CalcitTag, newTag, compareUnicodeStrings } from "./calcit-data.mjs";
import { CalcitList, CalcitSliceList } from "./js-list.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";
import { CalcitSet } from "./js-set.mjs";
import { CalcitRef } from "./js-ref.mjs";
import { CalcitEnumValue } from "./js-enum-value.mjs";
import { CalcitCirruQuote } from "./js-cirru.mjs";
import { CalcitTrait } from "./js-trait.mjs";

/// Shallow runtime check of a value against a struct field type form, matching
/// the native `value_matches_type_annotation`: container kinds are checked,
/// members are not. Names that cannot be resolved here (aliases, traits, type
/// variables) stay permissive only for the legacy, statically checked path.
/// Dynamic writes require evidence: deep contracts use generated validators,
/// while this fallback accepts only completely checkable scalar/open forms.
export let valueMatchesTypeForm = (value: CalcitValue, form: CalcitValue, requireEvidence = false): boolean =>
  matchTypeForm(value, form, requireEvidence) === true;

// null means that the contract cannot be checked here, not a mismatched value.
// Nullable absence proves its own branch; present payloads still require
// evidence for the inner contract.
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
        return value === null ? true : matchTypeForm(value, items[2], requireEvidence);
      }
      if (name === "JsNullish") {
        return value == null ? true : matchTypeForm(value, items[2], requireEvidence);
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

// Primitive spellings share one identity; unresolved names retain their form.
// Keep the original field form for diagnostics and runtime validation.
const typeNameAliases = new Map<string, string>();
for (const name of ["Dynamic", "Nil", "Unit", "Bool", "Number", "String", "Symbol", "Tag", "List", "Map", "Set", "Ref",
  "Fn", "Buffer", "CirruQuote", "JsObject", "Struct", "StructDef", "Enum", "EnumDef", "Trait", "Impl",
  "Int8", "UInt8", "Int16", "UInt16", "Int32", "UInt32", "Int64", "UInt64", "Float32", "Float64"]) {
  typeNameAliases.set(name, name);
  typeNameAliases.set(name.toLowerCase(), name);
}
for (const [alias, name] of [["any", "Dynamic"], ["Macro", "Fn"], ["macro", "Fn"], ["record", "Struct"], ["Record", "Struct"],
  ["tuple", "Enum"], ["Tuple", "Enum"], ["cirru-quote", "CirruQuote"], ["js-object", "JsObject"],
  ["struct-def", "StructDef"], ["enum-def", "EnumDef"]]) typeNameAliases.set(alias, name);

const containerTypeArity = new Map([["List", 1], ["Map", 2], ["Set", 1], ["Ref", 1]]);
const containerTypeIdentity = (name: string, args: CalcitValue[], generics: readonly string[]): CalcitValue => {
  const arity = containerTypeArity.get(name)!;
  return new CalcitSliceList([new CalcitSymbol("::"), new CalcitSymbol(name),
    ...Array.from({ length: arity }, (_, index) => typeFormIdentity(index < args.length ? args[index] : new CalcitSymbol("Dynamic"), generics))]);
};

const formItems = (form: CalcitValue, head: string): CalcitValue[] => {
  if (!(form instanceof CalcitList || form instanceof CalcitSliceList)) return [];
  const items = form.toArray();
  return typeFormName(items[0]) === head ? items.slice(1) : items;
};

const formPairs = (schema: CalcitValue): Array<[CalcitValue, CalcitValue]> | null => {
  if (schema instanceof CalcitMap || schema instanceof CalcitSliceMap) {
    return schema.pairs();
  } else if (schema instanceof CalcitList || schema instanceof CalcitSliceList) {
    const items = schema.toArray();
    if (typeFormName(items[0]) !== "{}") return null;
    const pairs: Array<[CalcitValue, CalcitValue]> = [];
    for (const item of items.slice(1)) {
      if (!(item instanceof CalcitList || item instanceof CalcitSliceList) || item.len() !== 2) return null;
      pairs.push(item.toArray() as [CalcitValue, CalcitValue]);
    }
    return pairs;
  } else {
    return null;
  }
};

export const typeBoundsIdentity = (form: CalcitValue, generics: readonly string[]): CalcitValue => {
  const bounds: Array<[string, CalcitValue]> = [];
  for (const [key, value] of formPairs(form) ?? []) {
    const name = typeFormName(key);
    if (name == null || !generics.includes(name)) continue;
    const quoted = (value instanceof CalcitList || value instanceof CalcitSliceList)
      && typeFormName(value.toArray()[0]) === "quote";
    const traits = value instanceof CalcitTrait || value instanceof CalcitSymbol || quoted ? [value] : formItems(value, "[]");
    if (traits.length === 0) continue;
    bounds.push([name, new CalcitSliceList(traits.map(trait => typeFormIdentity(trait, generics)))]);
  }
  bounds.sort(([left], [right]) => compareUnicodeStrings(left, right));
  return bounds.length === 0 ? null
    : new CalcitSliceList(bounds.map(([name, traits]) => new CalcitSliceList([name, traits])));
};

const functionTypeFormIdentity = (schema: CalcitValue, generics: readonly string[]): CalcitValue => {
  const pairs = formPairs(schema);
  if (pairs == null) return schema;
  const source = new Map<string, CalcitValue>();
  for (const [key, value] of pairs) {
    const name = typeFormName(key);
    if (name != null && !source.has(name)) source.set(name, value);
  }
  if (!["args", "return", "rest", "generics", "where", "kind", "features", "async"].some(key => source.has(key))) {
    return new CalcitSymbol("Fn");
  }
  const localGenerics = formItems(source.get("generics"), "[]").map(typeFormName).filter((name): name is string => name != null);
  const scope = [...generics, ...localGenerics];
  // Native function annotations materialize these defaults. Missing fields and
  // explicit defaults must denote the same callback contract.
  const fields = new Map<string, CalcitValue>([
    ["args", new CalcitSliceList([])], ["return", new CalcitSymbol("Dynamic")],
    ["kind", newTag("fn")], ["features", new CalcitSet([])],
    ["generics", new CalcitSliceList(localGenerics.map(name => new CalcitSymbol(name)))],
    ["where", typeBoundsIdentity(source.get("where"), scope)],
  ]);
  for (const [name, original] of source) {
    if (name === "return" || name === "rest") fields.set(name, typeFormIdentity(original, scope));
    if (name === "args") fields.set(name, new CalcitSliceList(formItems(original, "[]").map(form => typeFormIdentity(form, scope))));
    if (name === "kind") fields.set(name, newTag(typeFormName(original) === "macro" ? "macro" : "fn"));
    if (name === "features") {
      const items = original instanceof CalcitSet ? original.values() : formItems(original, "#{}");
      fields.set(name, new CalcitSet(items.filter(item => item instanceof CalcitTag && item.value !== "$async-invocation")));
    }
  }
  const async = source.get("async");
  if (async !== undefined && async !== null && async !== false) {
    fields.set("features", new CalcitSet([...(fields.get("features") as CalcitSet).values(), newTag("$async-invocation")]));
  }
  return new CalcitSliceMap([...fields].flatMap(([name, value]) => [newTag(name), value]));
};

export const typeFormIdentity = (form: CalcitValue, generics: readonly string[] = []): CalcitValue => {
  if (form === null) return new CalcitSymbol("Dynamic");
  if (form instanceof CalcitList || form instanceof CalcitSliceList) {
    const items = form.toArray();
    if (typeFormName(items[0]) === "quote" && items.length === 2) return typeFormIdentity(items[1], generics);
    if (typeFormName(items[0]) === "::") {
      const rawName = typeFormName(items[1]);
      const name = typeNameAliases.get(rawName) ?? rawName;
      if (items.length === 2 && typeNameAliases.has(rawName)) return typeFormIdentity(items[1], generics);
      if (containerTypeArity.has(name) && items.length - 2 <= containerTypeArity.get(name)) {
        return containerTypeIdentity(name, items.slice(2), generics);
      }
      if ((rawName === "Fn" || rawName === "fn") && items.length === 3) {
        const signature = functionTypeFormIdentity(items[2], generics);
        return signature instanceof CalcitSymbol && signature.value === "Fn" ? signature
          : new CalcitSliceList([new CalcitSymbol("::"), new CalcitSymbol("Fn"), signature]);
      }
    }
    return new CalcitSliceList(items.map(item => typeFormIdentity(item, generics)));
  }
  const name = typeFormName(form);
  const canonical = name == null ? null : typeNameAliases.get(name);
  if (canonical == null) {
    return form instanceof CalcitSymbol && form.typeRef != null && !generics.includes(form.value)
      ? new CalcitSymbol(form.typeRef) : form;
  }
  return containerTypeArity.has(canonical) ? containerTypeIdentity(canonical, [], generics) : new CalcitSymbol(canonical);
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
