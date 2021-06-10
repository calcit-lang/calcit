import * as ternaryTree from "@calcit/ternary-tree";

import {
  TernaryTreeList,
  TernaryTreeMap,
  overwriteComparator,
  initTernaryTreeList,
  initTernaryTreeMap,
  listLen,
  mapLen,
  listGet,
  mapGet,
  assocMap,
  assocList,
  dissocMap,
  isMapEmpty,
  toPairs,
  contains,
  listToItems,
  dissocList,
  Hash,
  overwriteHashGenerator,
  valueHash,
  mergeValueHash,
  toPairsArray,
  assocBefore,
  assocAfter,
  mapGetDefault,
} from "@calcit/ternary-tree";

/** need to compare by Calcit */
let DATA_EQUAL = (x: CrDataValue, y: CrDataValue): boolean => {
  return x == y;
};

export let overwriteDataComparator = (f: typeof DATA_EQUAL): void => {
  DATA_EQUAL = f;
};

export class CrDataKeyword {
  value: string;
  cachedHash: Hash;
  constructor(x: string) {
    this.value = x;
  }
  toString() {
    return `:${this.value}`;
  }
}

export class CrDataSymbol {
  value: string;
  cachedHash: Hash;
  constructor(x: string) {
    this.value = x;
  }
  toString() {
    return `'${this.value}`;
  }
}

export class CrDataRecur {
  args: CrDataValue[];
  constructor(xs: CrDataValue[]) {
    this.args = xs;
  }

  toString() {
    return `(&recur ...)`;
  }
}

export let isNestedCrData = (x: CrDataValue): boolean => {
  if (x instanceof CrDataList) {
    return x.len() > 0;
  }
  if (x instanceof CrDataMap) {
    return x.len() > 0;
  }
  if (x instanceof CrDataRecord) {
    return x.fields.length > 0;
  }
  if (x instanceof CrDataSet) {
    return false;
  }
  return false;
};

export let tipNestedCrData = (x: CrDataValue): string => {
  if (x instanceof CrDataList) {
    return "'[]...";
  }
  if (x instanceof CrDataMap) {
    return "'{}...";
  }
  if (x instanceof CrDataRecord) {
    return "'%{}...";
  }
  if (x instanceof CrDataSet) {
    return "'#{}...";
  }
  return x.toString();
};

export class CrDataRef {
  value: CrDataValue;
  path: string;
  listeners: Map<CrDataValue, CrDataFn>;
  cachedHash: Hash;
  constructor(x: CrDataValue, path: string) {
    this.value = x;
    this.path = path;
    this.listeners = new Map();
  }
  toString(): string {
    return `(&ref ${this.value.toString()})`;
  }
}

export class CrDataTuple {
  fst: CrDataValue;
  snd: CrDataValue;
  cachedHash: Hash;
  constructor(a: CrDataValue, b: CrDataValue) {
    this.fst = a;
    this.snd = b;
  }
  get(n: number) {
    if (n == 0) {
      return this.fst;
    } else if (n == 1) {
      return this.snd;
    } else {
      throw new Error("Tuple only have 2 elements");
    }
  }
  toString(): string {
    return `(&tuple ${this.fst.toString()} ${this.snd.toString()})`;
  }
}

export type CrDataFn = (...xs: CrDataValue[]) => CrDataValue;

export class CrDataList {
  value: TernaryTreeList<CrDataValue>;
  // array mode store bare array for performance
  arrayValue: Array<CrDataValue>;
  arrayMode: boolean;
  arrayStart: number;
  arrayEnd: number;
  cachedHash: Hash;
  constructor(value: Array<CrDataValue> | TernaryTreeList<CrDataValue>) {
    if (Array.isArray(value)) {
      this.arrayMode = true;
      this.arrayValue = value;
      this.arrayStart = 0;
      this.arrayEnd = value.length;
      this.value = null;
    } else {
      this.arrayMode = false;
      this.value = value;
      this.arrayValue = [];
      this.arrayStart = null;
      this.arrayEnd = null;
    }
  }
  turnListMode() {
    if (this.arrayMode) {
      this.value = initTernaryTreeList(this.arrayValue.slice(this.arrayStart, this.arrayEnd));
      this.arrayValue = null;
      this.arrayStart = null;
      this.arrayEnd = null;
      this.arrayMode = false;
    }
  }
  len() {
    if (this.arrayMode) {
      return this.arrayEnd - this.arrayStart;
    } else {
      return listLen(this.value);
    }
  }
  get(idx: number) {
    if (this.arrayMode) {
      return this.arrayValue[this.arrayStart + idx];
    } else {
      return listGet(this.value, idx);
    }
  }
  assoc(idx: number, v: CrDataValue) {
    this.turnListMode();
    return new CrDataList(assocList(this.value, idx, v));
  }
  assocBefore(idx: number, v: CrDataValue) {
    this.turnListMode();
    return new CrDataList(assocBefore(this.value, idx, v));
  }
  assocAfter(idx: number, v: CrDataValue) {
    this.turnListMode();
    return new CrDataList(assocAfter(this.value, idx, v));
  }
  dissoc(idx: number) {
    this.turnListMode();
    return new CrDataList(dissocList(this.value, idx));
  }
  slice(from: number, to: number) {
    if (this.arrayMode) {
      if (from < 0) {
        throw new Error(`from index too small: ${from}`);
      }
      if (to > this.len()) {
        throw new Error(`end index too large: ${to}`);
      }
      if (to < from) {
        throw new Error("end index too small");
      }
      let result = new CrDataList(this.arrayValue);
      result.arrayStart = this.arrayStart + from;
      result.arrayEnd = this.arrayStart + to;
      return result;
    } else {
      return new CrDataList(ternaryTree.slice(this.value, from, to));
    }
  }
  toString(shorter = false): string {
    let result = "";
    for (let item of this.items()) {
      if (shorter && isNestedCrData(item)) {
        result = `${result} ${tipNestedCrData(item)}`;
      } else {
        result = `${result} ${toString(item, true)}`;
      }
    }
    return `([]${result})`;
  }
  isEmpty() {
    return this.len() === 0;
  }
  /** usage: `for of` */
  items(): Generator<CrDataValue> {
    if (this.arrayMode) {
      return sliceGenerator(this.arrayValue, this.arrayStart, this.arrayEnd);
    } else {
      return listToItems(this.value);
    }
  }
  append(v: CrDataValue) {
    if (this.arrayMode && this.arrayEnd === this.arrayValue.length && this.arrayStart < 32) {
      // dirty trick to reuse list memory, data storage actually appended at existing array
      this.arrayValue.push(v);
      let newList = new CrDataList(this.arrayValue);
      newList.arrayStart = this.arrayStart;
      newList.arrayEnd = this.arrayEnd + 1;
      return newList;
    } else {
      this.turnListMode();
      return new CrDataList(ternaryTree.append(this.value, v));
    }
  }
  prepend(v: CrDataValue) {
    this.turnListMode();
    return new CrDataList(ternaryTree.prepend(this.value, v));
  }
  first() {
    if (this.arrayMode) {
      if (this.arrayValue.length > this.arrayStart) {
        return this.arrayValue[this.arrayStart];
      } else {
        return null;
      }
    } else {
      return ternaryTree.first(this.value);
    }
  }
  rest() {
    if (this.arrayMode) {
      return this.slice(1, this.arrayEnd - this.arrayStart);
    } else {
      return new CrDataList(ternaryTree.rest(this.value));
    }
  }
  concat(ys: CrDataList) {
    if (!(ys instanceof CrDataList)) {
      throw new Error("Expected list");
    }
    if (this.arrayMode && ys.arrayMode) {
      let size = this.arrayEnd - this.arrayStart;
      let otherSize = ys.arrayEnd - ys.arrayStart;
      let combined = new Array(size + otherSize);
      for (let i = 0; i < size; i++) {
        combined[i] = this.get(i);
      }
      for (let i = 0; i < otherSize; i++) {
        combined[i + size] = ys.get(i);
      }
      return new CrDataList(combined);
    } else {
      this.turnListMode();
      ys.turnListMode();
      return new CrDataList(ternaryTree.concat(this.value, ys.value));
    }
  }
  map(f: (v: CrDataValue) => CrDataValue): CrDataList {
    if (this.arrayMode) {
      return new CrDataList(this.arrayValue.slice(this.arrayStart, this.arrayEnd).map(f));
    } else {
      return new CrDataList(ternaryTree.listMapValues(this.value, f));
    }
  }
  toArray(): CrDataValue[] {
    if (this.arrayMode) {
      return this.arrayValue.slice(this.arrayStart, this.arrayEnd);
    } else {
      return [...ternaryTree.listToItems(this.value)];
    }
  }
  reverse() {
    this.turnListMode();
    return new CrDataList(ternaryTree.reverse(this.value));
  }
}

export class CrDataMap {
  cachedHash: Hash;
  /** in arrayMode, only flatten values, not tree structure */
  arrayMode: boolean;
  arrayValue: CrDataValue[];
  value: TernaryTreeMap<CrDataValue, CrDataValue>;
  skipValue: CrDataValue;
  constructor(value: CrDataValue[] | TernaryTreeMap<CrDataValue, CrDataValue>) {
    if (Array.isArray(value)) {
      this.arrayMode = true;
      this.arrayValue = value;
    } else {
      this.arrayMode = false;
      this.value = value;
    }
  }
  turnMap() {
    if (this.arrayMode) {
      var dict: Array<[CrDataValue, CrDataValue]> = [];
      let halfLength = this.arrayValue.length >> 1;
      for (let idx = 0; idx < halfLength; idx++) {
        dict.push([this.arrayValue[idx << 1], this.arrayValue[(idx << 1) + 1]]);
      }
      this.value = initTernaryTreeMap(dict);
      this.arrayMode = false;
      this.arrayValue = null;
    }
  }
  len() {
    if (this.arrayMode) {
      return this.arrayValue.length >> 1;
    } else {
      return mapLen(this.value);
    }
  }
  get(k: CrDataValue) {
    if (this.arrayMode && this.arrayValue.length <= 16) {
      let size = this.arrayValue.length >> 1;
      for (let i = 0; i < size; i++) {
        let pos = i << 1;
        if (DATA_EQUAL(this.arrayValue[pos], k)) {
          return this.arrayValue[pos + 1];
        }
      }
      return null;
    } else {
      this.turnMap();
      return mapGetDefault(this.value, k, null);
    }
  }
  assoc(k: CrDataValue, v: CrDataValue) {
    if (this.arrayMode && this.arrayValue.length <= 16) {
      let ret = this.arrayValue.slice(0);
      for (let i = 0; i < ret.length; i += 2) {
        if (DATA_EQUAL(k, ret[i])) {
          ret[i + 1] = v;
          return new CrDataMap(ret);
        }
      }
      ret.push(k, v);
      return new CrDataMap(ret);
    } else {
      this.turnMap();
      return new CrDataMap(assocMap(this.value, k, v));
    }
  }
  dissoc(k: CrDataValue) {
    if (this.arrayMode && this.arrayValue.length <= 16) {
      let ret: CrDataValue[] = [];
      for (let i = 0; i < this.arrayValue.length; i += 2) {
        if (!DATA_EQUAL(k, this.arrayValue[i])) {
          ret.push(this.arrayValue[i], this.arrayValue[i + 1]);
        }
      }
      return new CrDataMap(ret);
    } else {
      this.turnMap();
      return new CrDataMap(dissocMap(this.value, k));
    }
  }
  toString(shorter = false) {
    let itemsCode = "";
    for (let [k, v] of this.pairs()) {
      if (shorter) {
        let keyPart = isNestedCrData(k) ? tipNestedCrData(k) : toString(k, true);
        let valuePart = isNestedCrData(v) ? tipNestedCrData(v) : toString(v, true);
        itemsCode = `${itemsCode} (${keyPart} ${valuePart})`;
      } else {
        itemsCode = `${itemsCode} (${toString(k, true)} ${toString(v, true)})`;
      }
    }
    return `({}${itemsCode})`;
  }
  isEmpty() {
    if (this.arrayMode) {
      return this.arrayValue.length == 0;
    } else {
      return isMapEmpty(this.value);
    }
  }
  pairs(): Array<[CrDataValue, CrDataValue]> {
    if (this.arrayMode) {
      let ret: Array<[CrDataValue, CrDataValue]> = [];
      let size = this.arrayValue.length >> 1;
      for (let i = 0; i < size; i++) {
        let pos = i << 1;
        ret.push([this.arrayValue[pos], this.arrayValue[pos + 1]]);
      }
      return ret;
    } else {
      return toPairsArray(this.value);
    }
  }
  contains(k: CrDataValue) {
    if (this.arrayMode && this.arrayValue.length <= 16) {
      // guessed number
      let size = this.arrayValue.length >> 1;
      for (let i = 0; i < size; i++) {
        let pos = i << 1;
        if (DATA_EQUAL(this.arrayValue[pos], k)) {
          return true;
        }
      }
      return false;
    } else {
      this.turnMap();
      return ternaryTree.contains(this.value, k);
    }
  }
  merge(ys: CrDataMap) {
    return this.mergeSkip(ys, null);
  }
  mergeSkip(ys: CrDataMap, v: CrDataValue) {
    if (ys == null) {
      return this;
    }

    if (!(ys instanceof CrDataMap)) {
      console.error("value:", v);
      throw new Error("Expected map to merge");
    }

    if (this.arrayMode && ys.arrayMode && this.arrayValue.length + ys.arrayValue.length <= 24) {
      // probably this length < 16, ys length < 8
      let ret = this.arrayValue.slice(0);
      outer: for (let i = 0; i < ys.arrayValue.length; i = i + 2) {
        if (ys.arrayValue[i + 1] == v) {
          continue;
        }
        for (let k = 0; k < ret.length; k = k + 2) {
          if (DATA_EQUAL(ys.arrayValue[i], ret[k])) {
            ret[k + 1] = ys.arrayValue[i + 1];
            continue outer;
          }
        }
        ret.push(ys.arrayValue[i], ys.arrayValue[i + 1]);
      }
      return new CrDataMap(ret);
    }

    this.turnMap();

    if (ys.arrayMode) {
      let ret = this.value;
      let size = ys.arrayValue.length >> 1;
      for (let i = 0; i < size; i++) {
        let pos = i << 1;
        if (ys.arrayValue[pos + 1] == v) {
          continue;
        }
        ret = assocMap(ret, ys.arrayValue[pos], ys.arrayValue[pos + 1]);
      }
      return new CrDataMap(ret);
    } else {
      return new CrDataMap(ternaryTree.mergeSkip(this.value, ys.value, v));
    }
  }
}

export let getStringName = (x: CrDataValue): string => {
  if (typeof x === "string") {
    return x;
  }
  if (x instanceof CrDataKeyword) {
    return x.value;
  }
  if (x instanceof CrDataSymbol) {
    return x.value;
  }
  throw new Error("Cannot get string as name");
};

/** returns -1 when not found */
export function findInFields(xs: Array<string>, y: string): number {
  let lower = 0;
  let upper = xs.length - 1;

  while (upper - lower > 1) {
    let pos = (lower + upper) >> 1;
    let v = xs[pos];
    if (y < v) {
      upper = pos - 1;
    } else if (y > v) {
      lower = pos + 1;
    } else {
      return pos;
    }
  }

  if (y == xs[lower]) return lower;
  if (y == xs[upper]) return upper;
  return -1;
}

export class CrDataRecord {
  name: string;
  fields: Array<string>;
  values: Array<CrDataValue>;
  constructor(name: string, fields: Array<CrDataValue>, values?: Array<CrDataValue>) {
    this.name = name;
    let fieldNames = fields.map(getStringName);
    this.fields = fieldNames;
    if (values != null) {
      if (values.length != fields.length) {
        throw new Error("value length not match");
      }
      this.values = values;
    } else {
      this.values = new Array(fieldNames.length);
    }
  }
  get(k: CrDataValue) {
    let field = getStringName(k);
    let idx = findInFields(this.fields, field);
    if (idx >= 0) {
      return this.values[idx];
    } else {
      throw new Error(`Cannot find :${field} among (${this.values.join(",")})`);
    }
  }
  assoc(k: CrDataValue, v: CrDataValue): CrDataRecord {
    let values: Array<CrDataValue> = new Array(this.fields.length);
    let name = getStringName(k);
    for (let idx in this.fields) {
      if (this.fields[idx] === name) {
        values[idx] = v;
      } else {
        values[idx] = this.values[idx];
      }
    }
    return new CrDataRecord(this.name, this.fields, values);
  }
  merge() {
    // TODO
  }
  toString(): string {
    let ret = "(%{} " + this.name;
    for (let idx in this.fields) {
      ret += " (" + this.fields[idx] + " " + toString(this.values[idx], true) + ")";
    }
    return ret + ")";
  }
}

export type CrDataValue =
  | string
  | number
  | boolean
  | CrDataMap
  | CrDataList
  | CrDataSet
  | CrDataKeyword
  | CrDataSymbol
  | CrDataRef
  | CrDataTuple
  | CrDataFn
  | CrDataRecur // should not be exposed to function
  | CrDataRecord
  | null;

var keywordRegistery: Record<string, CrDataKeyword> = {};

export let kwd = (content: string) => {
  let item = keywordRegistery[content];
  if (item != null) {
    return item;
  } else {
    let v = new CrDataKeyword(content);
    keywordRegistery[content] = v;
    return v;
  }
};

export var refsRegistry = new Map<string, CrDataRef>();

let defaultHash_nil = valueHash("nil:");
let defaultHash_number = valueHash("number:");
let defaultHash_string = valueHash("string:");
let defaultHash_keyword = valueHash("keyword:");
let defaultHash_true = valueHash("true:");
let defaultHash_false = valueHash("false:");
let defaultHash_symbol = valueHash("symbol:");
let defaultHash_fn = valueHash("fn:");
let defaultHash_ref = valueHash("ref:");
let defaultHash_tuple = valueHash("tuple:");
let defaultHash_set = valueHash("set:");
let defaultHash_list = valueHash("list:");
let defaultHash_map = valueHash("map:");

let fnHashCounter = 0;

let hashFunction = (x: CrDataValue): Hash => {
  if (x == null) {
    return defaultHash_nil;
  }
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
  if (x instanceof CrDataKeyword) {
    let h = mergeValueHash(defaultHash_keyword, x.value);
    x.cachedHash = h;
    return h;
  }
  if (x === true) {
    return defaultHash_true;
  }
  if (x === false) {
    return defaultHash_false;
  }
  if (x instanceof CrDataSymbol) {
    let h = mergeValueHash(defaultHash_symbol, x.value);
    x.cachedHash = h;
    return h;
  }
  if (typeof x === "function") {
    fnHashCounter = fnHashCounter + 1;
    let h = mergeValueHash(defaultHash_fn, fnHashCounter);
    (x as any).cachedHash = h;
    return h;
  }
  if (x instanceof CrDataRef) {
    let h = mergeValueHash(defaultHash_ref, x.path);
    x.cachedHash = h;
    return h;
  }
  if (x instanceof CrDataTuple) {
    let base = defaultHash_list;
    base = mergeValueHash(base, hashFunction(x.fst));
    base = mergeValueHash(base, hashFunction(x.snd));
    x.cachedHash = base;
    return base;
  }
  if (x instanceof CrDataSet) {
    // TODO not using dirty solution for code
    let base = defaultHash_set;
    for (let item of x.value) {
      base = mergeValueHash(base, hashFunction(item));
    }
    return base;
  }
  if (x instanceof CrDataList) {
    let base = defaultHash_list;
    for (let item of x.items()) {
      base = mergeValueHash(base, hashFunction(item));
    }
    x.cachedHash = base;
    return base;
  }
  if (x instanceof CrDataMap) {
    let base = defaultHash_map;
    for (let [k, v] of x.pairs()) {
      base = mergeValueHash(base, hashFunction(k));
      base = mergeValueHash(base, hashFunction(v));
    }
    x.cachedHash = base;
    return base;
  }
  throw new Error("Unknown data for hashing");
};

// Dirty code to change ternary-tree behavior
overwriteHashGenerator(hashFunction);

function* sliceGenerator(xs: Array<CrDataValue>, start: number, end: number): Generator<CrDataValue> {
  for (let idx = start; idx < end; idx++) {
    yield xs[idx];
  }
}

export let toString = (x: CrDataValue, escaped: boolean): string => {
  if (x == null) {
    return "nil";
  }
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
    return `(&fn ...)`;
  }
  if (x instanceof CrDataSymbol) {
    return x.toString();
  }
  if (x instanceof CrDataKeyword) {
    return x.toString();
  }
  if (x instanceof CrDataList) {
    return x.toString();
  }
  if (x instanceof CrDataMap) {
    return x.toString();
  }
  if (x instanceof CrDataSet) {
    return x.toString();
  }
  if (x instanceof CrDataRecord) {
    return x.toString();
  }
  if (x instanceof CrDataRef) {
    return x.toString();
  }
  if (x instanceof CrDataTuple) {
    return x.toString();
  }

  console.warn("Unknown structure to string, better use `console.log`", x);
  return `${x}`;
};

export let cloneSet = (xs: Set<CrDataValue>): Set<CrDataValue> => {
  if (!(xs instanceof Set)) {
    throw new Error("Expected a set");
  }
  var result: Set<CrDataValue> = new Set();
  for (let v of xs) {
    result.add(v);
  }
  return result;
};

export class CrDataSet {
  value: Set<CrDataValue>;
  constructor(value: Set<CrDataValue>) {
    this.value = value;
  }
  len() {
    return this.value.size;
  }
  contains(y: CrDataValue) {
    return this.value.has(y);
  }
  include(y: CrDataValue): CrDataSet {
    var result = cloneSet(this.value);
    result.add(y);
    return new CrDataSet(result);
  }
  exclude(y: CrDataValue): CrDataSet {
    var result = cloneSet(this.value);
    result.delete(y);
    return new CrDataSet(result);
  }

  difference(ys: CrDataSet): CrDataSet {
    var result = cloneSet(this.value);
    ys.value.forEach((y) => {
      if (result.has(y)) {
        result.delete(y);
      }
    });
    return new CrDataSet(result);
  }
  union(ys: CrDataSet): CrDataSet {
    var result = cloneSet(this.value);
    ys.value.forEach((y) => {
      if (!result.has(y)) {
        result.add(y);
      }
    });
    return new CrDataSet(result);
  }
  intersection(ys: CrDataSet): CrDataSet {
    let xs = this.value;
    var result: Set<CrDataValue> = new Set();
    ys.value.forEach((y) => {
      if (xs.has(y)) {
        result.add(y);
      }
    });
    return new CrDataSet(result);
  }

  first(): CrDataValue {
    // rather suspicious solution since set has no logic order
    if (this.value.size === 0) {
      return null;
    }
    for (let x of this.value) {
      return x;
    }
  }
  rest(): CrDataSet {
    if (this.value.size == 0) {
      return null;
    }
    let x0 = this.first();
    let ys = cloneSet(this.value);
    ys.delete(x0);
    return new CrDataSet(ys);
  }

  toString() {
    let itemsCode = "";
    this.value.forEach((child, idx) => {
      itemsCode = `${itemsCode} ${toString(child, true)}`;
    });
    return `(#{}${itemsCode})`;
  }

  values() {
    return this.value.values();
  }
}
