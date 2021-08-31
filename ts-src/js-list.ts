import * as ternaryTree from "@calcit/ternary-tree";

import { CalcitValue } from "./js-primes";

import {
  TernaryTreeList,
  initTernaryTreeList,
  listLen,
  listGet,
  assocList,
  listToItems,
  dissocList,
  Hash,
  assocBefore,
  assocAfter,
} from "@calcit/ternary-tree";

import { CalcitMap } from "./js-map";
import { CalcitSet } from "./js-set";
import { CalcitTuple } from "./js-tuple";
import { CalcitFn } from "./calcit.procs";

import { isNestedCalcitData, tipNestedCalcitData, toString } from "./calcit-data";

export class CalcitList {
  value: TernaryTreeList<CalcitValue>;
  // array mode store bare array for performance
  arrayValue: Array<CalcitValue>;
  arrayMode: boolean;
  arrayStart: number;
  arrayEnd: number;
  cachedHash: Hash;
  constructor(value: Array<CalcitValue> | TernaryTreeList<CalcitValue>) {
    if (value == null) {
      value = []; // dirty, better handled from outside
    }
    this.cachedHash = null;
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
  assoc(idx: number, v: CalcitValue) {
    this.turnListMode();
    return new CalcitList(assocList(this.value, idx, v));
  }
  assocBefore(idx: number, v: CalcitValue) {
    this.turnListMode();
    return new CalcitList(assocBefore(this.value, idx, v));
  }
  assocAfter(idx: number, v: CalcitValue) {
    this.turnListMode();
    return new CalcitList(assocAfter(this.value, idx, v));
  }
  dissoc(idx: number) {
    this.turnListMode();
    return new CalcitList(dissocList(this.value, idx));
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
      let result = new CalcitList(this.arrayValue);
      result.arrayStart = this.arrayStart + from;
      result.arrayEnd = this.arrayStart + to;
      return result;
    } else {
      return new CalcitList(ternaryTree.slice(this.value, from, to));
    }
  }
  toString(shorter = false): string {
    let result = "";
    for (let item of this.items()) {
      if (shorter && isNestedCalcitData(item)) {
        result = `${result} ${tipNestedCalcitData(item)}`;
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
  items(): Generator<CalcitValue> {
    if (this.arrayMode) {
      return sliceGenerator(this.arrayValue, this.arrayStart, this.arrayEnd);
    } else {
      return listToItems(this.value);
    }
  }
  append(v: CalcitValue) {
    if (this.arrayMode && this.arrayEnd === this.arrayValue.length && this.arrayStart < 32) {
      // dirty trick to reuse list memory, data storage actually appended at existing array
      this.arrayValue.push(v);
      let newList = new CalcitList(this.arrayValue);
      newList.arrayStart = this.arrayStart;
      newList.arrayEnd = this.arrayEnd + 1;
      return newList;
    } else {
      this.turnListMode();
      return new CalcitList(ternaryTree.append(this.value, v));
    }
  }
  prepend(v: CalcitValue) {
    this.turnListMode();
    return new CalcitList(ternaryTree.prepend(this.value, v));
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
      return new CalcitList(ternaryTree.rest(this.value));
    }
  }
  concat(ys: CalcitList) {
    if (!(ys instanceof CalcitList)) {
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
      return new CalcitList(combined);
    } else {
      this.turnListMode();
      ys.turnListMode();
      return new CalcitList(ternaryTree.concat(this.value, ys.value));
    }
  }
  map(f: (v: CalcitValue) => CalcitValue): CalcitList {
    if (this.arrayMode) {
      return new CalcitList(this.arrayValue.slice(this.arrayStart, this.arrayEnd).map(f));
    } else {
      return new CalcitList(ternaryTree.listMapValues(this.value, f));
    }
  }
  toArray(): CalcitValue[] {
    if (this.arrayMode) {
      return this.arrayValue.slice(this.arrayStart, this.arrayEnd);
    } else {
      return [...ternaryTree.listToItems(this.value)];
    }
  }
  reverse() {
    this.turnListMode();
    return new CalcitList(ternaryTree.reverse(this.value));
  }
}

function* sliceGenerator(xs: Array<CalcitValue>, start: number, end: number): Generator<CalcitValue> {
  for (let idx = start; idx < end; idx++) {
    yield xs[idx];
  }
}

export let foldl = function (xs: CalcitValue, acc: CalcitValue, f: CalcitFn): CalcitValue {
  if (arguments.length !== 3) {
    throw new Error("foldl takes 3 arguments");
  }

  if (f == null) {
    debugger;
    throw new Error("Expected function for folding");
  }
  if (xs instanceof CalcitList) {
    var result = acc;
    for (let idx = 0; idx < xs.len(); idx++) {
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
  if (xs instanceof CalcitMap) {
    let result = acc;
    xs.pairs().forEach(([k, item]) => {
      result = f(result, new CalcitList([k, item]));
    });
    return result;
  }
  throw new Error("Unknow data for foldl");
};

export let foldl_shortcut = function (xs: CalcitValue, acc: CalcitValue, v0: CalcitValue, f: CalcitFn): CalcitValue {
  if (arguments.length !== 4) {
    throw new Error("foldl-shortcut takes 4 arguments");
  }

  if (f == null) {
    debugger;
    throw new Error("Expected function for folding");
  }
  if (xs instanceof CalcitList) {
    var state = acc;
    for (let idx = 0; idx < xs.len(); idx++) {
      let item = xs.get(idx);
      let pair = f(state, item);
      if (pair instanceof CalcitTuple) {
        if (typeof pair.fst === "boolean") {
          if (pair.fst) {
            return pair.snd;
          } else {
            state = pair.snd;
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
    for (let item of xs.values()) {
      let pair = f(state, item);
      if (pair instanceof CalcitTuple) {
        if (typeof pair.fst === "boolean") {
          if (pair.fst) {
            return pair.snd;
          } else {
            state = pair.snd;
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
    for (let item of xs.pairs()) {
      let pair = f(state, new CalcitList(item));
      if (pair instanceof CalcitTuple) {
        if (typeof pair.fst === "boolean") {
          if (pair.fst) {
            return pair.snd;
          } else {
            state = pair.snd;
          }
        }
      } else {
        throw new Error("Expected return value in `:: bool acc` structure");
      }
    }
    return v0;
  }
  throw new Error("Unknow data for foldl-shortcut");
};
