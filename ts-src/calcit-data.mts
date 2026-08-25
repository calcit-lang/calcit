import { Hash, overwriteHashGenerator, valueHash, mergeValueHash } from "@calcit/ternary-tree";
import { overwriteComparator, initTernaryTreeMap } from "@calcit/ternary-tree";
import { overwriteMapComparator } from "./js-map.mjs";
import { disableListStructureCheck } from "@calcit/ternary-tree";

import { CalcitStructValue, fieldsEqual } from "./js-struct-value.mjs";
import { CalcitImpl } from "./js-impl.mjs";
import { CalcitStructDef } from "./js-struct-def.mjs";
import { CalcitEnumDef } from "./js-enum-def.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";

import { CalcitValue, _$n_compare } from "./js-primes.mjs";
import { CalcitList, CalcitSliceList } from "./js-list.mjs";
import { CalcitSet, overwriteSetComparator } from "./js-set.mjs";
import { CalcitEnumValue } from "./js-enum-value.mjs";
import { CalcitTrait } from "./js-trait.mjs";
import { CalcitCirruQuote, cirru_deep_equal } from "./js-cirru.mjs";
import { CirruWriterNode } from "@cirru/writer.ts";
import { CalcitRef } from "./js-ref.mjs";

// we have to inject cache in a dirty way in some cases
const calcit_dirty_hash_key = "_calcit_cached_hash";

let tagIdx = 0;

export class CalcitTag {
  value: string;
  cachedHash: Hash;
  // use tag for fast comparing
  idx: number;
  constructor(x: string) {
    this.value = x;
    this.idx = tagIdx;
    tagIdx++;
    this.cachedHash = null;
  }
  toString() {
    return `:${this.value}`;
  }
  cmp(other: CalcitTag): number {
    if (this.idx < other.idx) {
      return -1;
    } else if (this.idx > other.idx) {
      return 1;
    } else {
      return 0;
    }
  }
}

export class CalcitSymbol {
  value: string;
  cachedHash: Hash;
  constructor(x: string) {
    this.value = x;
    this.cachedHash = null;
  }
  toString() {
    return `'${this.value}`;
  }
}

export class CalcitRecur {
  args: CalcitValue[];
  cachedHash: Hash;
  constructor(xs: CalcitValue[]) {
    this.args = xs;
    this.cachedHash = null;
  }

  toString() {
    return `(&recur ...)`;
  }
}

export let isNestedCalcitData = (x: CalcitValue): boolean => {
  if (x instanceof CalcitList || x instanceof CalcitSliceList) {
    return x.len() > 0;
  }
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) {
    return x.len() > 0;
  }
  if (x instanceof CalcitStructValue) {
    return x.fields.length > 0;
  }
  if (x instanceof CalcitImpl) {
    return x.fields.length > 0;
  }
  if (x instanceof CalcitSet) {
    return false;
  }
  return false;
};

export let tipNestedCalcitData = (x: CalcitValue): string => {
  if (x instanceof CalcitList || x instanceof CalcitSliceList) {
    return "'[]...";
  }
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) {
    return "'{}...";
  }
  if (x instanceof CalcitStructValue) {
    return "'%{}...";
  }
  if (x instanceof CalcitImpl) {
    return "'%impl...";
  }
  if (x instanceof CalcitSet) {
    return "'#{}...";
  }
  return x.toString();
};

export type CalcitFn = (...xs: CalcitValue[]) => CalcitValue;

export let getStringName = (x: CalcitValue): string => {
  if (typeof x === "string") {
    return x;
  }
  if (x instanceof CalcitTag) {
    return x.value;
  }
  if (x instanceof CalcitSymbol) {
    return x.value;
  }
  throw new Error("Cannot get string as name");
};

/** Compare tag names by Unicode scalar value, matching Rust `str` ordering. */
export function compareTagNames(x: CalcitTag, y: CalcitTag): number {
  let xIdx = 0;
  let yIdx = 0;
  while (xIdx < x.value.length && yIdx < y.value.length) {
    const xCode = x.value.codePointAt(xIdx)!;
    const yCode = y.value.codePointAt(yIdx)!;
    if (xCode < yCode) return -1;
    if (xCode > yCode) return 1;
    xIdx += xCode > 0xffff ? 2 : 1;
    yIdx += yCode > 0xffff ? 2 : 1;
  }
  if (xIdx < x.value.length) return 1;
  if (yIdx < y.value.length) return -1;
  return 0;
}

export function tagNamesAreCanonical(fields: CalcitTag[]): boolean {
  for (let idx = 1; idx < fields.length; idx++) {
    if (compareTagNames(fields[idx - 1], fields[idx]) >= 0) return false;
  }
  return true;
}

export function canonicalizeTagPairs<T>(fields: CalcitTag[], values: T[], context: string): [CalcitTag[], T[]] {
  if (fields.length !== values.length) {
    throw new Error(`${context}: fields and values length mismatch`);
  }
  if (tagNamesAreCanonical(fields)) return [fields, values];

  const pairs = fields.map((field, idx) => [field, values[idx]] as [CalcitTag, T]);
  pairs.sort((a, b) => compareTagNames(a[0], b[0]));
  for (let idx = 1; idx < pairs.length; idx++) {
    if (pairs[idx - 1][0].value === pairs[idx][0].value) {
      throw new Error(`${context}: duplicated field :${pairs[idx][0].value}`);
    }
  }
  return [
    pairs.map(([field]) => field),
    pairs.map(([, value]) => value),
  ];
}

/** returns -1 when not found */
export function findInFields(xs: Array<CalcitTag>, y: CalcitTag): number {
  let low = 0;
  let high = xs.length - 1;

  while (low <= high) {
    const mid = Math.floor((low + high) / 2);
    const midVal = xs[mid];
    const ordering = compareTagNames(midVal, y);
    if (ordering < 0) {
      low = mid + 1;
    } else if (ordering > 0) {
      high = mid - 1;
    } else {
      return mid;
    }
  }
  return -1;
}

var tagRegistry: Record<string, CalcitTag> = {};

export let newTag = (content: string) => {
  let item = tagRegistry[content];
  if (item != null) {
    return item;
  } else {
    let v = new CalcitTag(content);
    tagRegistry[content] = v;
    return v;
  }
};

export let castTag = (x: CalcitValue): CalcitTag => {
  if (x instanceof CalcitTag) {
    return x;
  }
  if (typeof x === "string") {
    return newTag(x);
  }
  if (x instanceof CalcitSymbol) {
    return newTag(x.value);
  }
  if (typeof x === "function") {
    const methodName = (x as { __calcitMethodName?: unknown }).__calcitMethodName;
    if (typeof methodName === "string") {
      return newTag(methodName);
    }
  }
  throw new Error(`Cannot cast this to tag: ${x}`);
};

export var refsRegistry = new Map<string, CalcitRef>();

let defaultHash_nil = valueHash("nil:");
let defaultHash_unit = valueHash("unit:");
let defaultHash_number = valueHash("number:");
let defaultHash_string = valueHash("string:");
let defaultHash_tag = valueHash("tag:");
let defaultHash_true = valueHash("bool:true");
let defaultHash_false = valueHash("bool:false");
let defaultHash_symbol = valueHash("symbol:");
let defaultHash_fn = valueHash("fn:");
let defaultHash_ref = valueHash("ref:");
let defaultHash_tuple = valueHash("tuple:");
let defaultHash_set = valueHash("set:");
let defaultHash_list = valueHash("list:");
let defaultHash_map = valueHash("map:");
let defaultHash_record = valueHash("record:");
let defaultHash_impl = valueHash("impl:");
let defaultHash_struct = valueHash("struct:");
let defaultHash_enum = valueHash("enum:");
let defaultHash_cirru_quote = valueHash("cirru-quote:");

let defaultHash_unknown = valueHash("unknown:");

let fnHashCounter = 0;
let jsObjectHashCounter = 0;

export let hashFunction = (x: CalcitValue): Hash => {
  if (x === null) {
    return defaultHash_nil;
  }
  if (x === undefined) return defaultHash_unit;
  if (typeof x === "number") {
    return mergeValueHash(defaultHash_number, x);
  }
  if (typeof x === "string") {
    return mergeValueHash(defaultHash_string, x);
  }
  // dirty solution of caching, trying to reduce cost
  if ((x as any).cachedHash != null) {
    return (x as any).cachedHash;
  }
  if ((x as any)[calcit_dirty_hash_key] != null) {
    return (x as any)[calcit_dirty_hash_key];
  }

  if (x instanceof CalcitTag) {
    let h = mergeValueHash(defaultHash_tag, x.idx);
    x.cachedHash = h;
    return h;
  }
  if (x === true) {
    return defaultHash_true;
  }
  if (x === false) {
    return defaultHash_false;
  }
  if (x instanceof CalcitSymbol) {
    let h = mergeValueHash(defaultHash_symbol, x.value);
    x.cachedHash = h;
    return h;
  }
  if (typeof x === "function") {
    // method values are closures created on the fly (see invoke_method_closure);
    // hash by method name so equal methods share the same hash, matching isEqual
    const methodName = (x as { __calcitMethodName?: string }).__calcitMethodName;
    if (methodName != null) {
      let h = mergeValueHash(defaultHash_fn, methodName);
      (x as any)[calcit_dirty_hash_key] = h;
      return h;
    }
    fnHashCounter = fnHashCounter + 1;
    let h = mergeValueHash(defaultHash_fn, fnHashCounter);
    (x as any)[calcit_dirty_hash_key] = h;
    return h;
  }
  if (x instanceof CalcitRef) {
    let h = mergeValueHash(defaultHash_ref, x.path);
    x.cachedHash = h;
    return h;
  }
  if (x instanceof CalcitEnumValue) {
    let base = defaultHash_tuple;
    base = mergeValueHash(base, hashFunction(x.tag));
    for (let idx = 0; idx < x.extra.length; idx++) {
      let item = x.extra[idx];
      base = mergeValueHash(base, hashFunction(item));
    }
    x.cachedHash = base;
    return base;
  }
  if (x instanceof CalcitSet) {
    let base = defaultHash_set;
    let values = x.values();
    // sort elements for stable hash result
    values.sort((a, b) => _$n_compare(a, b));
    for (let idx = 0; idx < values.length; idx++) {
      let item = values[idx];
      base = mergeValueHash(base, hashFunction(item));
    }
    return base;
  }
  if (x instanceof CalcitSliceList) {
    let base = defaultHash_list;
    // low-level code for perf
    for (let idx = x.start; idx < x.end; idx++) {
      let item = x.value[idx];
      base = mergeValueHash(base, hashFunction(item));
    }
    x.cachedHash = base;
    return base;
  }
  if (x instanceof CalcitList) {
    let base = defaultHash_list;
    for (let item of x.items()) {
      base = mergeValueHash(base, hashFunction(item));
    }
    x.cachedHash = base;
    return base;
  }
  if (x instanceof CalcitSliceMap) {
    let base = defaultHash_map;
    let pairs = x.pairs();
    pairs.sort((a, b) => _$n_compare(a[0], b[0]));
    for (let idx = 0; idx < pairs.length; idx++) {
      let k = pairs[idx][0];
      let v = pairs[idx][1];
      base = mergeValueHash(base, hashFunction(k));
      base = mergeValueHash(base, hashFunction(v));
    }
    x.cachedHash = base;
    return base;
  }
  if (x instanceof CalcitMap) {
    let base = defaultHash_map;

    let pairs = x.pairs();
    pairs.sort((a, b) => _$n_compare(a[0], b[0]));
    for (let idx = 0; idx < pairs.length; idx++) {
      let k = pairs[idx][0];
      let v = pairs[idx][1];
      base = mergeValueHash(base, hashFunction(k));
      base = mergeValueHash(base, hashFunction(v));
    }
    x.cachedHash = base;
    return base;
  }
  if (x instanceof CalcitStructValue) {
    let base = defaultHash_record;
    for (let idx = 0; idx < x.fields.length; idx++) {
      base = mergeValueHash(base, hashFunction(x.fields[idx]));
      base = mergeValueHash(base, hashFunction(x.values[idx]));
    }
    x.cachedHash = base;
    return base;
  }
  if (x instanceof CalcitImpl) {
    let base = defaultHash_impl;
    base = mergeValueHash(base, hashFunction(x.name));
    if (x.origin != null) {
      base = mergeValueHash(base, hashFunction(x.origin));
    }
    for (let idx = 0; idx < x.fields.length; idx++) {
      base = mergeValueHash(base, hashFunction(x.fields[idx]));
      base = mergeValueHash(base, hashFunction(x.values[idx]));
    }
    x.cachedHash = base;
    return base;
  }
  if (x instanceof CalcitStructDef) {
    let base = defaultHash_struct;
    base = mergeValueHash(base, hashFunction(x.name));
    for (let idx = 0; idx < x.fields.length; idx++) {
      base = mergeValueHash(base, hashFunction(x.fields[idx]));
      base = mergeValueHash(base, hashFunction(x.fieldTypes[idx]));
    }
    for (let impl of x.impls) {
      base = mergeValueHash(base, hashFunction(impl));
    }
    x.cachedHash = base;
    return base;
  }
  if (x instanceof CalcitEnumDef) {
    let base = defaultHash_enum;
    base = mergeValueHash(base, hashFunction(x.prototype));
    for (let impl of x.impls) {
      base = mergeValueHash(base, hashFunction(impl));
    }
    x.cachedHash = base;
    return base;
  }
  if (x instanceof CalcitCirruQuote) {
    let base = defaultHash_cirru_quote;
    base = hashCirru(base, x.value);
    return base;
  }
  console.warn(`[warn] calcit-js has no method for hashing this: ${x}`);
  // currently we use dirty solution here to generate a custom hash
  // probably happening in .to-pairs of maps, putting a js object into a set
  // better forbid this, use .to-list instead
  let hashJsObject = defaultHash_unknown;
  jsObjectHashCounter = jsObjectHashCounter + 1;
  hashJsObject = mergeValueHash(hashJsObject, jsObjectHashCounter);
  (x as any)[calcit_dirty_hash_key] = hashJsObject;
  return hashJsObject;
};

/// traverse Cirru tree to make unique hash
let hashCirru = (base: number, x: CirruWriterNode) => {
  if (typeof x === "string") {
    return mergeValueHash(base, hashFunction(x));
  } else {
    for (let idx = 0; idx < x.length; idx++) {
      base = mergeValueHash(base, hashCirru(base, x[idx]));
    }
    return base;
  }
};

// Dirty code to change ternary-tree behavior
overwriteHashGenerator(hashFunction);

export let toString = (x: CalcitValue, escaped: boolean, disableJsDataWarning: boolean = false): string => {
  if (x === null) {
    return "nil";
  }
  if (x === undefined) return "&unit";
  if (typeof x === "string") {
    if (escaped) {
      // turn to visual string representation
      if (/[\)\(\s\"]/.test(x)) {
        return JSON.stringify("|" + x);
      } else {
        return "|" + x;
      }
    } else {
      return x;
    }
  }
  if (typeof x === "number") {
    return x.toString();
  }
  if (typeof x === "boolean") {
    return x.toString();
  }
  if (typeof x === "function") {
    const methodName = (x as { __calcitMethodName?: string }).__calcitMethodName;
    if (methodName != null) {
      return "." + methodName;
    }
    return `(&fn ...)`;
  }
  if (x instanceof CalcitSymbol) {
    return x.toString();
  }
  if (x instanceof CalcitTag) {
    return x.toString();
  }
  if (x instanceof CalcitList || x instanceof CalcitSliceList) {
    return x.toString(false, disableJsDataWarning);
  }
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) {
    return x.toString(false, disableJsDataWarning);
  }
  if (x instanceof CalcitSet) {
    return x.toString(disableJsDataWarning);
  }
  if (x instanceof CalcitStructValue) {
    return x.toString(disableJsDataWarning);
  }
  if (x instanceof CalcitImpl) {
    return x.toString(disableJsDataWarning);
  }
  if (x instanceof CalcitStructDef) {
    return x.toString(disableJsDataWarning);
  }
  if (x instanceof CalcitEnumDef) {
    return x.toString();
  }
  if (x instanceof CalcitTrait) {
    return x.toString(disableJsDataWarning);
  }
  if (x instanceof CalcitRef) {
    return x.toString();
  }
  if (x instanceof CalcitEnumValue) {
    return x.toString(disableJsDataWarning);
  }
  if (x instanceof CalcitCirruQuote) {
    return x.toString();
  }

  if (!disableJsDataWarning) {
    console.warn("Non Calcit data in stringify", x);
  }
  return `(#js ${JSON.stringify(x)})`;
};

export let to_js_data = (x: CalcitValue, options?: CalcitValue | boolean): any => {
  let addColon = false;
  if (typeof options === "boolean") {
    console.warn("to-js-data: the addColon boolean argument is deprecated; pass an options map instead");
    addColon = options;
  } else if (options !== undefined) {
    let jsOptions = to_js_data_inner(options, false) as Record<string, any>;
    addColon = jsOptions[":add-colon"] === true || jsOptions["add-colon"] === true;
    let value = to_js_data_inner(x, addColon);
    let expectedType =
      jsOptions[":type"] ??
      jsOptions.type ??
      (jsOptions[":js-array"] || jsOptions["js-array"] ? "js-array" : undefined) ??
      (jsOptions[":js-string"] || jsOptions["js-string"] ? "js-string" : undefined) ??
      (jsOptions[":js-number"] || jsOptions["js-number"] ? "js-number" : undefined) ??
      (jsOptions[":js-object"] || jsOptions["js-object"] ? "js-object" : undefined) ??
      "js-object";
    let valid =
      (expectedType === "js-array" && Array.isArray(value)) ||
      (expectedType === "js-object" &&
        value !== null &&
        typeof value === "object" &&
        !Array.isArray(value) &&
        !(value instanceof Set)) ||
      (expectedType === "js-string" && typeof value === "string") ||
      (expectedType === "js-number" && typeof value === "number");
    if (!valid) throw new Error(`to-js-data expects ${expectedType}`);
    return value;
  }
  return to_js_data_inner(x, addColon);
};

let to_js_data_inner = (x: CalcitValue, addColon: boolean): any => {
  if (x === null) {
    return null;
  }
  if (x === undefined) return undefined;
  if (x === true || x === false) {
    return x;
  }
  if (typeof x === "string") {
    return x;
  }
  if (typeof x === "number") {
    return x;
  }
  if (typeof x === "function") {
    return x;
  }
  if (x instanceof CalcitTag) {
    if (addColon) {
      return `:${x.value}`;
    }
    return x.value;
  }
  if (x instanceof CalcitSymbol) {
    if (addColon) {
      return `:${x.value}`;
    }
    return Symbol(x.value);
  }
  if (x instanceof CalcitEnumValue) {
    var result: any[] = [to_js_data_inner(x.tag, false)];
    for (let i = 0; i < x.extra.length; i++) {
      let item = x.extra[i];
      result.push(to_js_data_inner(item, false));
    }
    return result;
  }
  if (x instanceof CalcitList || x instanceof CalcitSliceList) {
    var result: any[] = [];
    for (let item of x.items()) {
      result.push(to_js_data_inner(item, addColon));
    }
    return result;
  }
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) {
    let result: Record<string, CalcitValue> = {};
    let pairs = x.pairs();
    for (let idx = 0; idx < pairs.length; idx++) {
      let k = pairs[idx][0];
      let v = pairs[idx][1];
      var key = to_js_data_inner(k, addColon);
      result[key] = to_js_data_inner(v, addColon);
    }
    return result;
  }
  if (x instanceof CalcitSet) {
    let result = new Set();
    x.values().forEach((v) => {
      result.add(to_js_data_inner(v, addColon));
    });
    return result;
  }
  if (x instanceof CalcitStructValue) {
    let result: Record<string, CalcitValue> = {};
    for (let idx = 0; idx < x.fields.length; idx++) {
      result[x.fields[idx].value] = to_js_data_inner(x.values[idx], false);
    }
    return result;
  }
  if (x instanceof CalcitRef) {
    throw new Error("Cannot convert ref to plain data");
  }
  if (x instanceof CalcitRecur) {
    throw new Error("Cannot convert recur to plain data");
  }

  return x;
};

export let _$n_map_$o_get = function (xs: CalcitValue, k: CalcitValue) {
  if (arguments.length !== 2) {
    throw new Error("map &get takes 2 arguments");
  }

  if (xs instanceof CalcitMap || xs instanceof CalcitSliceMap) return xs.get(k);

  throw new Error("Does not support `&get` on this type");
};

export let _$n__$e_ = (x: CalcitValue, y: CalcitValue): boolean => {
  if (x === y) {
    return true;
  }
  if (x === null) {
    return false;
  }

  let tx = typeof x;
  let ty = typeof y;

  if (tx !== ty) {
    return false;
  }

  if (tx === "string") {
    // already checked above
    return false;
  }
  if (tx === "boolean") {
    // already checked above
    return false;
  }
  if (tx === "number") {
    // already checked above
    return false;
  }
  if (tx === "function") {
    // method values are closures created on the fly (see invoke_method_closure),
    // so two methods with the same name must compare equal by name, not by reference
    const mx = (x as { __calcitMethodName?: string }).__calcitMethodName;
    const my = (y as { __calcitMethodName?: string }).__calcitMethodName;
    if (mx != null || my != null) {
      return mx === my;
    }
    // comparing plain functions by reference
    return x === y;
  }
  if (x instanceof CalcitTag) {
    // comparing tags by reference
    // already checked above
    return false;
  }
  if (x instanceof CalcitSymbol) {
    if (y instanceof CalcitSymbol) {
      return x.value === y.value;
    }
    return false;
  }
  if (x instanceof CalcitCirruQuote) {
    if (y instanceof CalcitCirruQuote) {
      return cirru_deep_equal(x.value, y.value);
    }
    return false;
  }
  if (x instanceof CalcitList || x instanceof CalcitSliceList) {
    if (y instanceof CalcitList || y instanceof CalcitSliceList) {
      if (x.len() !== y.len()) {
        return false;
      }
      let size = x.len();
      for (let idx = 0; idx < size; idx++) {
        let xItem = x.get(idx);
        let yItem = y.get(idx);
        if (!_$n__$e_(xItem, yItem)) {
          return false;
        }
      }
      return true;
    }
    return false;
  }
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) {
    if (y instanceof CalcitMap || y instanceof CalcitSliceMap) {
      if (x.len() !== y.len()) {
        return false;
      }
      let pairs = x.pairs();
      for (let idx = 0; idx < pairs.length; idx++) {
        let k = pairs[idx][0];
        let v = pairs[idx][1];
        if (!y.contains(k)) {
          return false;
        }
        if (!_$n__$e_(v, _$n_map_$o_get(y, k))) {
          return false;
        }
      }
      return true;
    }
    return false;
  }
  if (x instanceof CalcitRef) {
    if (y instanceof CalcitRef) {
      return x.path === y.path;
    }
    return false;
  }
  if (x instanceof CalcitEnumValue) {
    if (y instanceof CalcitEnumValue) {
      return x.eq(y);
    }
    return false;
  }
  if (x instanceof CalcitSet) {
    if (y instanceof CalcitSet) {
      if (x.len() !== y.len()) {
        return false;
      }
      let values = x.values();
      let yValues = y.values();
      // Optimize: create a Map for O(1) lookup instead of O(n) iteration
      let yMap = new Map();
      for (let idx = 0; idx < yValues.length; idx++) {
        let yv = yValues[idx];
        yMap.set(yv, true);
      }

      for (let idx = 0; idx < values.length; idx++) {
        let v = values[idx];
        let found = false;
        // First try direct lookup for primitive values
        if (yMap.has(v)) {
          found = true;
        } else {
          // Fallback to deep equality check for complex values
          for (let yv of yValues) {
            if (_$n__$e_(v, yv)) {
              found = true;
              break;
            }
          }
        }
        if (!found) {
          return false;
        }
      }
      return true;
    }
    return false;
  }
  if (x instanceof CalcitRecur) {
    if (y instanceof CalcitRecur) {
      console.warn("Do not compare Recur");
      return false;
    }
    return false;
  }
  if (x instanceof CalcitStructValue) {
    if (y instanceof CalcitStructValue) {
      if (x.name !== y.name) {
        return false;
      }
      if (!fieldsEqual(x.fields, y.fields)) {
        return false;
      }
      if (x.values.length !== y.values.length) {
        return false;
      }
      for (let idx = 0; idx < x.fields.length; idx++) {
        if (!_$n__$e_(x.values[idx], y.values[idx])) {
          return false;
        }
      }
      return true;
    }
    return false;
  }
  if (x instanceof CalcitImpl) {
    if (y instanceof CalcitImpl) {
      if (x.name !== y.name) {
        return false;
      }
      if ((x.origin == null) !== (y.origin == null)) {
        return false;
      }
      if (x.origin != null && y.origin != null && x.origin.name.value !== y.origin.name.value) {
        return false;
      }
      if (!fieldsEqual(x.fields, y.fields)) {
        return false;
      }
      if (x.values.length !== y.values.length) {
        return false;
      }
      for (let idx = 0; idx < x.fields.length; idx++) {
        if (!_$n__$e_(x.values[idx], y.values[idx])) {
          return false;
        }
      }
      return true;
    }
    return false;
  }
  if (x instanceof CalcitStructDef) {
    if (y instanceof CalcitStructDef) {
      return x.name === y.name && fieldsEqual(x.fields, y.fields);
    }
    return false;
  }
  if (x instanceof CalcitEnumDef) {
    if (y instanceof CalcitEnumDef) {
      return x.name === y.name;
    }
    return false;
  }
  if (x instanceof CalcitTrait) {
    if (y instanceof CalcitTrait) {
      return x.name === y.name;
    }
    return false;
  }
  throw new Error("Missing handler for this type");
};

// overwrite internary comparator of ternary-tree
overwriteComparator(_$n__$e_);
overwriteMapComparator(_$n__$e_);
overwriteSetComparator(_$n__$e_);

/** special trick for disabling ternary tree list check */
export let disable_list_structure_check_$x_ = disableListStructureCheck;
