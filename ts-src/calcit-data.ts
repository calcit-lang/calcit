import { Hash, overwriteHashGenerator, valueHash, mergeValueHash } from "@calcit/ternary-tree";
import { overwriteComparator, initTernaryTreeMap } from "@calcit/ternary-tree";
import { overwriteDataComparator } from "./js-map";

import { CrDataRecord, fieldsEqual } from "./js-record";
import { CrDataMap } from "./js-map";

import { CrDataValue } from "./js-primes";
import { CrDataList } from "./js-list";
import { CrDataSet } from "./js-set";
import { CrDataTuple } from "./js-tuple";

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

export type CrDataFn = (...xs: CrDataValue[]) => CrDataValue;

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
    let base = defaultHash_tuple;
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

export let to_js_data = (x: CrDataValue, addColon: boolean = false): any => {
  if (x == null) {
    return null;
  }
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
  if (x instanceof CrDataKeyword) {
    if (addColon) {
      return `:${x.value}`;
    }
    return x.value;
  }
  if (x instanceof CrDataSymbol) {
    if (addColon) {
      return `:${x.value}`;
    }
    return Symbol(x.value);
  }
  if (x instanceof CrDataList) {
    var result: any[] = [];
    for (let item of x.items()) {
      result.push(to_js_data(item, addColon));
    }
    return result;
  }
  if (x instanceof CrDataMap) {
    let result: Record<string, CrDataValue> = {};
    for (let [k, v] of x.pairs()) {
      var key = to_js_data(k, addColon);
      result[key] = to_js_data(v, addColon);
    }
    return result;
  }
  if (x instanceof CrDataSet) {
    let result = new Set();
    x.value.forEach((v) => {
      result.add(to_js_data(v, addColon));
    });
    return result;
  }
  if (x instanceof CrDataRecord) {
    let result: Record<string, CrDataValue> = {};
    for (let idx in x.fields) {
      result[x.fields[idx]] = to_js_data(x.values[idx]);
    }
    return result;
  }
  if (x instanceof CrDataRef) {
    throw new Error("Cannot convert ref to plain data");
  }
  if (x instanceof CrDataRecur) {
    throw new Error("Cannot convert recur to plain data");
  }

  return x;
};

export let _AND_map_COL_get = function (xs: CrDataValue, k: CrDataValue) {
  if (arguments.length !== 2) {
    throw new Error("map &get takes 2 arguments");
  }

  if (xs instanceof CrDataMap) return xs.get(k);

  throw new Error("Does not support `&get` on this type");
};

export let _AND__EQ_ = (x: CrDataValue, y: CrDataValue): boolean => {
  if (x === y) {
    return true;
  }
  if (x == null) {
    if (y == null) {
      return true;
    }
    return false;
  }

  let tx = typeof x;
  let ty = typeof y;

  if (tx !== ty) {
    return false;
  }

  if (tx === "string") {
    return (x as string) === (y as string);
  }
  if (tx === "boolean") {
    return (x as boolean) === (y as boolean);
  }
  if (tx === "number") {
    return x === y;
  }
  if (tx === "function") {
    // comparing functions by reference
    return x === y;
  }
  if (x instanceof CrDataKeyword) {
    if (y instanceof CrDataKeyword) {
      return x === y;
    }
    return false;
  }
  if (x instanceof CrDataSymbol) {
    if (y instanceof CrDataSymbol) {
      return x.value === y.value;
    }
    return false;
  }
  if (x instanceof CrDataList) {
    if (y instanceof CrDataList) {
      if (x.len() !== y.len()) {
        return false;
      }
      let size = x.len();
      for (let idx = 0; idx < size; idx++) {
        let xItem = x.get(idx);
        let yItem = y.get(idx);
        if (!_AND__EQ_(xItem, yItem)) {
          return false;
        }
      }
      return true;
    }
    return false;
  }
  if (x instanceof CrDataMap) {
    if (y instanceof CrDataMap) {
      if (x.len() !== y.len()) {
        return false;
      }
      for (let [k, v] of x.pairs()) {
        if (!y.contains(k)) {
          return false;
        }
        if (!_AND__EQ_(v, _AND_map_COL_get(y, k))) {
          return false;
        }
      }
      return true;
    }
    return false;
  }
  if (x instanceof CrDataRef) {
    if (y instanceof CrDataRef) {
      return x.path === y.path;
    }
    return false;
  }
  if (x instanceof CrDataTuple) {
    if (y instanceof CrDataTuple) {
      return _AND__EQ_(x.fst, y.fst) && _AND__EQ_(x.snd, y.snd);
    }
    return false;
  }
  if (x instanceof CrDataSet) {
    if (y instanceof CrDataSet) {
      if (x.len() !== y.len()) {
        return false;
      }
      for (let v of x.value) {
        let found = false;
        // testing by doing iteration is O(n2), could be slow
        // but Set::contains does not satisfy here
        for (let yv of y.value) {
          if (_AND__EQ_(v, yv)) {
            found = true;
            break;
          }
        }
        if (found) {
          continue;
        } else {
          return false;
        }
      }
      return true;
    }
    return false;
  }
  if (x instanceof CrDataRecur) {
    if (y instanceof CrDataRecur) {
      console.warn("Do not compare Recur");
      return false;
    }
    return false;
  }
  if (x instanceof CrDataRecord) {
    if (y instanceof CrDataRecord) {
      if (x.name !== y.name) {
        return false;
      }
      if (!fieldsEqual(x.fields, y.fields)) {
        return false;
      }
      if (x.values.length !== y.values.length) {
        return false;
      }
      for (let idx in x.fields) {
        if (!_AND__EQ_(x.values[idx], y.values[idx])) {
          return false;
        }
      }
      return true;
    }
    return false;
  }
  throw new Error("Missing handler for this type");
};

// overwrite internary comparator of ternary-tree
overwriteComparator(_AND__EQ_);
overwriteDataComparator(_AND__EQ_);
