import { Hash, overwriteHashGenerator, valueHash, mergeValueHash } from "@calcit/ternary-tree";
import { overwriteComparator, initTernaryTreeMap } from "@calcit/ternary-tree";
import { overwriteMapComparator } from "./js-map.mjs";

import { CalcitRecord, fieldsEqual } from "./js-record.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";

import { CalcitValue, _$n_compare } from "./js-primes.mjs";
import { CalcitList, CalcitSliceList } from "./js-list.mjs";
import { CalcitSet, overwriteSetComparator } from "./js-set.mjs";
import { CalcitTuple } from "./js-tuple.mjs";
import { CalcitCirruQuote, cirru_deep_equal } from "./js-cirru.mjs";
import { CirruWriterNode } from "@cirru/writer.ts";

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
  if (x instanceof CalcitRecord) {
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
  if (x instanceof CalcitRecord) {
    return "'%{}...";
  }
  if (x instanceof CalcitSet) {
    return "'#{}...";
  }
  return x.toString();
};

export class CalcitRef {
  value: CalcitValue;
  path: string;
  listeners: Map<CalcitValue, CalcitFn>;
  cachedHash: Hash;
  constructor(x: CalcitValue, path: string) {
    this.value = x;
    this.path = path;
    this.listeners = new Map();
    this.cachedHash = null;
  }
  toString(): string {
    return `(&ref ${this.value.toString()})`;
  }
}

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

/** returns -1 when not found */
export function findInFields(xs: Array<CalcitTag>, y: CalcitTag): number {
  let lower = 0;
  let upper = xs.length - 1;

  while (upper - lower > 1) {
    let pos = (lower + upper) >> 1;
    let v = xs[pos];
    if (y.idx < v.idx) {
      upper = pos - 1;
    } else if (y.idx > v.idx) {
      lower = pos + 1;
    } else {
      return pos;
    }
  }

  if (y === xs[lower]) return lower;
  if (y === xs[upper]) return upper;
  return -1;
}

var tagRegistery: Record<string, CalcitTag> = {};

export let newTag = (content: string) => {
  let item = tagRegistery[content];
  if (item != null) {
    return item;
  } else {
    let v = new CalcitTag(content);
    tagRegistery[content] = v;
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
  throw new Error(`Cannot cast this to tag: ${x}`);
};

export var refsRegistry = new Map<string, CalcitRef>();

let defaultHash_nil = valueHash("nil:");
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
let defaultHash_cirru_quote = valueHash("cirru-quote:");

let defaultHash_unknown = valueHash("unknown:");

let fnHashCounter = 0;
let jsObjectHashCounter = 0;

export let hashFunction = (x: CalcitValue): Hash => {
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
  if (x instanceof CalcitTuple) {
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
      let [k, v] = pairs[idx];
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
      let [k, v] = pairs[idx];
      base = mergeValueHash(base, hashFunction(k));
      base = mergeValueHash(base, hashFunction(v));
    }
    x.cachedHash = base;
    return base;
  }
  if (x instanceof CalcitRecord) {
    let base = defaultHash_record;
    for (let idx = 0; idx < x.fields.length; idx++) {
      base = mergeValueHash(base, hashFunction(x.fields[idx]));
      base = mergeValueHash(base, hashFunction(x.values[idx]));
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
  if (x instanceof CalcitRecord) {
    return x.toString(disableJsDataWarning);
  }
  if (x instanceof CalcitRef) {
    return x.toString();
  }
  if (x instanceof CalcitTuple) {
    return x.toString(disableJsDataWarning);
  }
  if (x instanceof CalcitCirruQuote) {
    return x.toString();
  }

  if (!disableJsDataWarning) {
    console.warn("Unknown structure to string, better use `console.log`", x);
  }
  return `(#js ${JSON.stringify(x)})`;
};

export let to_js_data = (x: CalcitValue, addColon: boolean = false): any => {
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
  if (x instanceof CalcitList || x instanceof CalcitSliceList) {
    var result: any[] = [];
    for (let item of x.items()) {
      result.push(to_js_data(item, addColon));
    }
    return result;
  }
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) {
    let result: Record<string, CalcitValue> = {};
    let pairs = x.pairs();
    for (let idx = 0; idx < pairs.length; idx++) {
      let [k, v] = pairs[idx];
      var key = to_js_data(k, addColon);
      result[key] = to_js_data(v, addColon);
    }
    return result;
  }
  if (x instanceof CalcitSet) {
    let result = new Set();
    x.values().forEach((v) => {
      result.add(to_js_data(v, addColon));
    });
    return result;
  }
  if (x instanceof CalcitRecord) {
    let result: Record<string, CalcitValue> = {};
    for (let idx = 0; idx < x.fields.length; idx++) {
      result[x.fields[idx].value] = to_js_data(x.values[idx]);
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
  if (x instanceof CalcitTag) {
    if (y instanceof CalcitTag) {
      return x === y;
    }
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
        let [k, v] = pairs[idx];
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
  if (x instanceof CalcitTuple) {
    if (y instanceof CalcitTuple) {
      return _$n__$e_(x.tag, y.tag) && _$n__$e_(x.get(1), y.get(1));
    }
    return false;
  }
  if (x instanceof CalcitSet) {
    if (y instanceof CalcitSet) {
      if (x.len() !== y.len()) {
        return false;
      }
      let values = x.values();
      for (let idx = 0; idx < values.length; idx++) {
        let v = values[idx];
        let found = false;
        // testing by doing iteration is O(n2), could be slow
        // but Set::contains does not satisfy here
        let yValues = y.values();
        for (let idx = 0; idx < yValues.length; idx++) {
          let yv = yValues[idx];
          if (_$n__$e_(v, yv)) {
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
  if (x instanceof CalcitRecur) {
    if (y instanceof CalcitRecur) {
      console.warn("Do not compare Recur");
      return false;
    }
    return false;
  }
  if (x instanceof CalcitRecord) {
    if (y instanceof CalcitRecord) {
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
  throw new Error("Missing handler for this type");
};

// overwrite internary comparator of ternary-tree
overwriteComparator(_$n__$e_);
overwriteMapComparator(_$n__$e_);
overwriteSetComparator(_$n__$e_);
