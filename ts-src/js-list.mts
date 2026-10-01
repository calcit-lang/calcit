import { FingerVec } from "@calcit/finger-vec";
import { Hash } from "@calcit/ternary-tree";

import { CalcitValue, isLiteral } from "./js-primes.mjs";

import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";
import { CalcitSet } from "./js-set.mjs";
import { CalcitEnumValue } from "./js-enum-value.mjs";

import { isNestedCalcitData, tipNestedCalcitData, toString, CalcitFn } from "./calcit-data.mjs";

// two list implementations, should offer same interface
export class CalcitList {
  value: FingerVec<CalcitValue>;
  cachedHash: Hash;
  constructor(value: FingerVec<CalcitValue>) {
    this.cachedHash = null;
    if (value == null) {
      this.value = FingerVec.empty();
    } else {
      this.value = value;
    }
  }
  len() {
    return this.value.size;
  }
  get(idx: number) {
    if (this.value.size === 0) {
      return null;
    }
    if (idx < 0 || idx >= this.value.size) {
      throw new Error(`Index ${idx} out of range for list of size ${this.value.size}`);
    }
    return this.value.get(idx);
  }
  assoc(idx: number, v: CalcitValue) {
    return new CalcitList(this.value.assoc(idx, v));
  }
  assocBefore(idx: number, v: CalcitValue) {
    return new CalcitList(this.value.assocBefore(idx, v));
  }
  assocAfter(idx: number, v: CalcitValue) {
    return new CalcitList(this.value.assocAfter(idx, v));
  }
  dissoc(idx: number) {
    return new CalcitList(this.value.dissoc(idx));
  }
  slice(from: number, to: number) {
    return new CalcitList(this.value.slice(from, to));
  }
  toString(shorter = false, disableJsDataWarning: boolean = false): string {
    let result = "";
    for (let item of this.items()) {
      if (shorter && isNestedCalcitData(item)) {
        result = `${result} ${tipNestedCalcitData(item)}`;
      } else {
        result = `${result} ${toString(item, true, disableJsDataWarning)}`;
      }
    }
    return `([]${result})`;
  }
  isEmpty() {
    return this.value.size === 0;
  }
  /** usage: `for of` */
  items(): Generator<CalcitValue> {
    return this.value.items();
  }
  append(v: CalcitValue) {
    return new CalcitList(this.value.pushRight(v));
  }
  prepend(v: CalcitValue) {
    return new CalcitList(this.value.pushLeft(v));
  }
  first() {
    if (this.value.size === 0) {
      throw new Error("Cannot get from empty list");
    }
    return this.value.first();
  }
  rest() {
    return new CalcitList(this.value.rest());
  }
  concat(ys: CalcitList | CalcitSliceList) {
    if (ys instanceof CalcitSliceList) {
      return new CalcitList(this.value.concat(ys.turnListMode().value));
    } else if (ys instanceof CalcitList) {
      return new CalcitList(this.value.concat(ys.value));
    } else {
      throw new Error(`Unknown data to concat: ${ys}`);
    }
  }
  map(f: (v: CalcitValue) => CalcitValue): CalcitList {
    return new CalcitList(this.value.map(f));
  }
  toArray(): CalcitValue[] {
    return this.value.toArray();
  }
  reverse() {
    return new CalcitList(this.value.reverse());
  }
  nestedDataInChildren(): boolean {
    let size = this.len();
    for (let idx = 0; idx < size; idx++) {
      if (!isLiteral(this.get(idx))) {
        return true;
      }
    }
    return false;
  }
}

// represent append-only immutable list in Array slices
export class CalcitSliceList {
  // array mode store bare array for performance
  value: Array<CalcitValue>;
  start: number;
  end: number;
  cachedHash: Hash;
  /** reference to converted list */
  cachedTreeListRef: CalcitList;
  constructor(value: Array<CalcitValue>) {
    if (value == null) {
      value = []; // dirty, better handled from outside
    }
    this.cachedHash = null;

    this.value = value;
    this.start = 0;
    this.end = value.length;
  }
  turnListMode(): CalcitList {
    if (this.cachedTreeListRef != null) {
      return this.cachedTreeListRef;
    }
    let ret = new CalcitList(FingerVec.fromRange(this.value, this.start, this.end));
    this.cachedTreeListRef = ret;
    return ret;
  }
  len() {
    return this.end - this.start;
  }
  get(idx: number) {
    if (idx >= 0 && this.start + idx < this.end) {
      return this.value[this.start + idx];
    }
    return null;
  }
  assoc(idx: number, v: CalcitValue) {
    return this.turnListMode().assoc(idx, v);
  }
  assocBefore(idx: number, v: CalcitValue) {
    return this.turnListMode().assocBefore(idx, v);
  }
  assocAfter(idx: number, v: CalcitValue) {
    if (idx === this.len() - 1) {
      return this.append(v);
    } else {
      return this.turnListMode().assocAfter(idx, v);
    }
  }
  dissoc(idx: number) {
    if (idx === 0) {
      return this.rest();
    } else if (idx === this.len() - 1) {
      return this.slice(0, idx);
    } else {
      return this.turnListMode().dissoc(idx);
    }
  }
  slice(from: number, to: number) {
    if (from < 0) {
      throw new Error(`from index too small: ${from}`);
    }
    if (to > this.len()) {
      throw new Error(`end index too large: ${to}`);
    }
    if (to < from) {
      throw new Error("end index too small");
    }
    if (from === to) {
      // when it's empty, just return empty list
      return new CalcitSliceList([]);
    }
    let result = new CalcitSliceList(this.value);
    result.start = this.start + from;
    result.end = this.start + to;
    return result;
  }
  toString(shorter = false, disableJsDataWarning = false): string {
    let result = "";
    for (let item of this.items()) {
      if (shorter && isNestedCalcitData(item)) {
        result = `${result} ${tipNestedCalcitData(item)}`;
      } else {
        result = `${result} ${toString(item, true, disableJsDataWarning)}`;
      }
    }
    return `([]${result})`;
  }
  isEmpty() {
    return this.len() === 0;
  }
  /** usage: `for of` */
  items(): Generator<CalcitValue> {
    return sliceGenerator(this.value, this.start, this.end);
  }
  append(v: CalcitValue): CalcitSliceList | CalcitList {
    if (this.end === this.value.length && this.start < 32) {
      // dirty trick to reuse list memory, data storage actually appended at existing array
      this.value.push(v);
      let newList = new CalcitSliceList(this.value);
      newList.start = this.start;
      newList.end = this.end + 1;
      return newList;
    } else {
      return this.turnListMode().append(v);
    }
  }
  prepend(v: CalcitValue) {
    return this.turnListMode().prepend(v);
  }
  first() {
    if (this.value.length > this.start) {
      return this.value[this.start];
    } else {
      return null;
    }
  }
  rest() {
    return this.slice(1, this.end - this.start);
  }
  // TODO
  concat(ys: CalcitSliceList | CalcitList) {
    if (ys instanceof CalcitSliceList) {
      let size = this.end - this.start;
      let otherSize = ys.end - ys.start;
      let combined = new Array(size + otherSize);
      for (let i = 0; i < size; i++) {
        combined[i] = this.get(i);
      }
      for (let i = 0; i < otherSize; i++) {
        combined[i + size] = ys.get(i);
      }
      return new CalcitSliceList(combined);
    } else if (ys instanceof CalcitList) {
      return this.turnListMode().concat(ys);
    } else {
      throw new Error("Expected list");
    }
  }
  map(f: (v: CalcitValue) => CalcitValue): CalcitSliceList {
    let ys: CalcitValue[] = [];
    for (let idx = this.start; idx < this.end; idx++) {
      ys.push(f(this.value[idx]));
    }

    return new CalcitSliceList(ys);
  }
  toArray(): CalcitValue[] {
    return this.value.slice(this.start, this.end);
  }
  reverse() {
    return this.turnListMode().reverse();
  }
  nestedDataInChildren(): boolean {
    let size = this.len();
    for (let idx = 0; idx < size; idx++) {
      if (!isLiteral(this.get(idx))) {
        return true;
      }
    }
  }
}

function* sliceGenerator(xs: Array<CalcitValue>, start: number, end: number): Generator<CalcitValue> {
  if (xs == null) {
    if (end <= start) {
      throw new Error("invalid list to slice");
    }
  } else {
    for (let idx = start; idx < end; idx++) {
      yield xs[idx];
    }
  }
}

export let foldl = function (xs: CalcitValue, acc: CalcitValue, f: CalcitFn): CalcitValue {
  if (arguments.length !== 3) {
    throw new Error("foldl takes 3 arguments");
  }

  if (f == null) {
    throw new Error("Expected function for folding");
  }
  if (xs instanceof CalcitSliceList || xs instanceof CalcitList) {
    var result = acc;
    let size = xs.len();
    for (let idx = 0; idx < size; idx++) {
      let item = xs.get(idx);
      result = f(result, item);
    }
    return result;
  }
  if (xs instanceof CalcitSet) {
    let result = acc;
    xs.values().forEach((item) => {
      result = f(result, item);
    });
    return result;
  }
  if (xs instanceof CalcitSliceMap) {
    let result = acc;
    // low-level code for performance
    let size = xs.chunk.length >> 1;
    for (let i = 0; i < size; i++) {
      let pos = i << 1;
      result = f(result, new CalcitSliceList([xs.chunk[pos], xs.chunk[pos + 1]]));
    }
    return result;
  }
  if (xs instanceof CalcitMap) {
    let result = acc;
    xs.pairs().forEach((pair) => {
      result = f(result, new CalcitSliceList(pair));
    });
    return result;
  }
  throw new Error("Unknown data for foldl");
};

export let foldl_shortcut = function (xs: CalcitValue, acc: CalcitValue, v0: CalcitValue, f: CalcitFn): CalcitValue {
  if (arguments.length !== 4) {
    throw new Error("foldl-shortcut takes 4 arguments");
  }

  if (f == null) {
    throw new Error("Expected function for folding");
  }
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    var state = acc;
    let size = xs.len();
    for (let idx = 0; idx < size; idx++) {
      let item = xs.get(idx);
      let pair = f(state, item);
      if (pair instanceof CalcitEnumValue) {
        if (typeof pair.tag === "boolean") {
          if (pair.tag) {
            return pair.get(1);
          } else {
            state = pair.get(1);
          }
        }
      } else {
        throw new Error("Expected return value in `:: bool acc` structure");
      }
    }
    return v0;
  }
  if (xs instanceof CalcitSet) {
    let state = acc;
    let values = xs.values();
    for (let idx = 0; idx < values.length; idx++) {
      let item = values[idx];
      let pair = f(state, item);
      if (pair instanceof CalcitEnumValue) {
        if (typeof pair.tag === "boolean") {
          if (pair.tag) {
            return pair.get(1);
          } else {
            state = pair.get(1);
          }
        }
      } else {
        throw new Error("Expected return value in `:: bool acc` structure");
      }
    }
    return v0;
  }
  if (xs instanceof CalcitSliceMap) {
    let state = acc;
    // low-level code for performance
    let size = xs.chunk.length >> 1;
    for (let i = 0; i < size; i++) {
      let pos = i << 1;
      let pair = f(state, new CalcitSliceList([xs.chunk[pos], xs.chunk[pos + 1]]));
      if (pair instanceof CalcitEnumValue) {
        if (typeof pair.tag === "boolean") {
          if (pair.tag) {
            return pair.get(1);
          } else {
            state = pair.get(1);
          }
        }
      } else {
        throw new Error("Expected return value in `:: bool acc` structure");
      }
    }
    return v0;
  }
  if (xs instanceof CalcitMap) {
    let state = acc;
    let pairs = xs.pairs();
    for (let idx = 0; idx < pairs.length; idx++) {
      let item = pairs[idx];
      let pair = f(state, new CalcitSliceList(item));
      if (pair instanceof CalcitEnumValue) {
        if (typeof pair.tag === "boolean") {
          if (pair.tag) {
            return pair.get(1);
          } else {
            state = pair.get(1);
          }
        }
      } else {
        throw new Error("Expected return value in `:: bool acc` structure");
      }
    }
    return v0;
  }
  throw new Error("Unknown data for foldl-shortcut");
};
export let foldr_shortcut = function (xs: CalcitValue, acc: CalcitValue, v0: CalcitValue, f: CalcitFn): CalcitValue {
  if (arguments.length !== 4) {
    throw new Error("foldr-shortcut takes 4 arguments");
  }

  if (f == null) {
    throw new Error("Expected function for folding");
  }
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    var state = acc;
    // iterate from right
    for (let idx = xs.len() - 1; idx >= 0; idx--) {
      let item = xs.get(idx);
      let pair = f(state, item);
      if (pair instanceof CalcitEnumValue) {
        if (typeof pair.tag === "boolean") {
          if (pair.tag) {
            return pair.get(1);
          } else {
            state = pair.get(1);
          }
        }
      } else {
        throw new Error("Expected return value in `:: bool acc` structure");
      }
    }
    return v0;
  }

  throw new Error("Unknown data for foldr-shortcut, expected only list");
};
