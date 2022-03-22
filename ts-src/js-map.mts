import * as ternaryTree from "@calcit/ternary-tree";

import { CalcitValue } from "./js-primes.mjs";
import { CalcitSet } from "./js-set.mjs";

import {
  TernaryTreeMap,
  initTernaryTreeMap,
  mapLen,
  assocMap,
  dissocMap,
  isMapEmpty,
  Hash,
  toPairsArray,
  mapGetDefault,
  initEmptyTernaryTreeMap,
  initTernaryTreeMapFromArray,
} from "@calcit/ternary-tree";

import { isNestedCalcitData, tipNestedCalcitData, toString } from "./calcit-data.mjs";

/** need to compare by Calcit */
let DATA_EQUAL = (x: CalcitValue, y: CalcitValue): boolean => {
  return x === y;
};

export let overwriteMapComparator = (f: typeof DATA_EQUAL): void => {
  DATA_EQUAL = f;
};

// a reference that equals to no other value(mainly for telling from `null`)
let fakeUniqueSymbol = [] as any;

export class CalcitMap {
  cachedHash: Hash;
  value: TernaryTreeMap<CalcitValue, CalcitValue>;
  constructor(value: TernaryTreeMap<CalcitValue, CalcitValue>) {
    if (value == null) {
      this.value = initEmptyTernaryTreeMap();
    } else {
      this.value = value;
    }
  }
  len() {
    return mapLen(this.value);
  }
  get(k: CalcitValue) {
    return mapGetDefault(this.value, k, null);
  }
  assoc(...args: CalcitValue[]) {
    if (args.length % 2 !== 0) throw new Error("expected even arguments");
    let size = Math.floor(args.length / 2);

    let result = this.value;
    for (let idx = 0; idx < size; idx++) {
      let k = args[idx << 1];
      let v = args[(idx << 1) + 1];
      result = assocMap(result, k, v);
    }
    return new CalcitMap(result);
  }
  dissoc(...args: CalcitValue[]) {
    let ret = this.value;
    for (let idx = 0; idx < args.length; idx++) {
      ret = dissocMap(ret, args[idx]);
    }
    return new CalcitMap(ret);
  }
  toString(shorter = false) {
    let itemsCode = "";
    for (let [k, v] of this.pairs()) {
      if (shorter) {
        let keyPart = isNestedCalcitData(k) ? tipNestedCalcitData(k) : toString(k, true);
        let valuePart = isNestedCalcitData(v) ? tipNestedCalcitData(v) : toString(v, true);
        itemsCode = `${itemsCode} (${keyPart} ${valuePart})`;
      } else {
        itemsCode = `${itemsCode} (${toString(k, true)} ${toString(v, true)})`;
      }
    }
    return `({}${itemsCode})`;
  }
  isEmpty() {
    return isMapEmpty(this.value);
  }
  pairs(): Array<[CalcitValue, CalcitValue]> {
    return toPairsArray(this.value);
  }
  keysArray(): Array<CalcitValue> {
    return [...ternaryTree.toKeys(this.value)];
  }
  contains(k: CalcitValue) {
    return ternaryTree.contains(this.value, k);
  }
  merge(ys: CalcitMap | CalcitSliceMap) {
    return this.mergeSkip(ys, fakeUniqueSymbol);
  }
  mergeSkip(ys: CalcitMap | CalcitSliceMap, v: CalcitValue) {
    if (ys == null) {
      return this;
    }

    if (!(ys instanceof CalcitMap || ys instanceof CalcitSliceMap)) {
      console.error("value:", v);
      throw new Error("Expected map to merge");
    }

    if (ys instanceof CalcitSliceMap) {
      let ret = this.value;
      let size = ys.chunk.length >> 1;
      for (let i = 0; i < size; i++) {
        let pos = i << 1;
        if (ys.chunk[pos + 1] === v) {
          continue;
        }
        ret = assocMap(ret, ys.chunk[pos], ys.chunk[pos + 1]);
      }
      return new CalcitMap(ret);
    } else {
      return new CalcitMap(ternaryTree.mergeSkip(this.value, ys.value, v));
    }
  }

  /** TODO implement diff with low level code, opens opportunity for future optimizations */
  diffNew(ys: CalcitMap | CalcitSliceMap): CalcitMap {
    let zs = this.value;
    if (ys instanceof CalcitSliceMap) {
      let size = ys.chunk.length >> 1;
      for (let i = 0; i < size; i++) {
        let pos = i << 1;
        let k = ys.chunk[pos];
        if (ternaryTree.contains(zs, k)) {
          zs = ternaryTree.dissocMap(zs, k);
        }
      }
      return new CalcitMap(zs);
    } else if (ys instanceof CalcitMap) {
      let ysKeys = ys.keysArray();
      for (let i = 0; i < ysKeys.length; i++) {
        let k = ysKeys[i];
        if (ternaryTree.contains(zs, k)) {
          zs = ternaryTree.dissocMap(zs, k);
        }
      }
      return new CalcitMap(zs);
    } else {
      throw new Error("unknown data to diff");
    }
  }

  /** TODO implement diff with low level code, opens opportunity for future optimizations */
  diffKeys(ys: CalcitMap | CalcitSliceMap): CalcitSet {
    let ret: Array<CalcitValue> = [];
    let ks = this.keysArray();
    for (let i = 0; i < ks.length; i++) {
      let k = ks[i];
      if (!ys.contains(k)) {
        ret.push(k);
      }
    }
    return new CalcitSet(ret);
  }

  /** TODO implement diff with low level code, opens opportunity for future optimizations */
  commonKeys(ys: CalcitMap | CalcitSliceMap): CalcitSet {
    let ret: Array<CalcitValue> = [];
    let ks = this.keysArray();
    for (let i = 0; i < ks.length; i++) {
      let k = ks[i];
      if (ys.contains(k)) {
        ret.push(k);
      }
    }
    return new CalcitSet(ret);
  }
}

// store small map in linear array to reduce cost of building tree
export class CalcitSliceMap {
  cachedHash: Hash;
  /** in arrayMode, only flatten values, instead of tree structure */
  chunk: CalcitValue[];
  constructor(value: CalcitValue[]) {
    if (value == null) {
      this.chunk = [];
    } else if (Array.isArray(value)) {
      this.chunk = value;
    } else {
      throw new Error("unknown data for map");
    }
  }
  turnMap(): CalcitMap {
    var dict: Array<[CalcitValue, CalcitValue]> = [];
    let halfLength = this.chunk.length >> 1;
    for (let idx = 0; idx < halfLength; idx++) {
      dict.push([this.chunk[idx << 1], this.chunk[(idx << 1) + 1]]);
    }
    let value = initTernaryTreeMapFromArray(dict);
    return new CalcitMap(value);
  }
  len() {
    return this.chunk.length >> 1;
  }
  get(k: CalcitValue) {
    if (this.chunk.length <= 16) {
      let size = this.chunk.length >> 1;
      for (let i = 0; i < size; i++) {
        let pos = i << 1;
        if (DATA_EQUAL(this.chunk[pos], k)) {
          return this.chunk[pos + 1];
        }
      }
      return null;
    } else {
      return this.turnMap().get(k);
    }
  }
  assoc(...args: CalcitValue[]) {
    if (args.length % 2 !== 0) throw new Error("expected even arguments");
    let size = Math.floor(args.length / 2);
    if (this.chunk.length <= 16) {
      let ret = this.chunk.slice(0);
      outer: for (let j = 0; j < size; j++) {
        let k = args[j << 1];
        let v = args[(j << 1) + 1];
        for (let i = 0; i < ret.length; i += 2) {
          if (DATA_EQUAL(k, ret[i])) {
            ret[i + 1] = v;
            continue outer; // data recorded, goto next loop
          }
        }
        ret.push(k, v);
      }
      return new CalcitSliceMap(ret);
    } else {
      return this.turnMap().assoc(...args);
    }
  }
  dissoc(...args: CalcitValue[]) {
    if (this.chunk.length <= 16) {
      let ret: CalcitValue[] = [];
      outer: for (let i = 0; i < this.chunk.length; i += 2) {
        for (let j = 0; j < args.length; j++) {
          let k = args[j];
          if (DATA_EQUAL(k, this.chunk[i])) {
            continue outer;
          }
        }
        ret.push(this.chunk[i], this.chunk[i + 1]);
      }
      return new CalcitSliceMap(ret);
    } else {
      return this.turnMap().dissoc(...args);
    }
  }
  toString(shorter = false) {
    let itemsCode = "";
    for (let [k, v] of this.pairs()) {
      if (shorter) {
        let keyPart = isNestedCalcitData(k) ? tipNestedCalcitData(k) : toString(k, true);
        let valuePart = isNestedCalcitData(v) ? tipNestedCalcitData(v) : toString(v, true);
        itemsCode = `${itemsCode} (${keyPart} ${valuePart})`;
      } else {
        itemsCode = `${itemsCode} (${toString(k, true)} ${toString(v, true)})`;
      }
    }
    return `({}${itemsCode})`;
  }
  isEmpty() {
    return this.chunk.length === 0;
  }
  pairs(): Array<[CalcitValue, CalcitValue]> {
    let ret: Array<[CalcitValue, CalcitValue]> = [];
    let size = this.chunk.length >> 1;
    for (let i = 0; i < size; i++) {
      let pos = i << 1;
      ret.push([this.chunk[pos], this.chunk[pos + 1]]);
    }
    return ret;
  }
  keysArray(): Array<CalcitValue> {
    let ret: Array<CalcitValue> = [];
    let size = this.chunk.length >> 1;
    for (let i = 0; i < size; i++) {
      let pos = i << 1;
      ret.push(this.chunk[pos]);
    }
    return ret;
  }
  contains(k: CalcitValue) {
    if (this.chunk.length <= 16) {
      // guessed number
      let size = this.chunk.length >> 1;
      for (let i = 0; i < size; i++) {
        let pos = i << 1;
        if (DATA_EQUAL(this.chunk[pos], k)) {
          return true;
        }
      }
      return false;
    } else {
      return this.turnMap().contains(k);
    }
  }
  merge(ys: CalcitMap | CalcitSliceMap) {
    return this.mergeSkip(ys, fakeUniqueSymbol);
  }
  mergeSkip(ys: CalcitMap | CalcitSliceMap, v: CalcitValue) {
    if (ys == null) {
      return this;
    }

    if (!(ys instanceof CalcitMap || ys instanceof CalcitSliceMap)) {
      console.error("value:", v);
      throw new Error("Expected map to merge");
    }

    if (ys instanceof CalcitSliceMap && this.chunk.length + ys.len() <= 24) {
      // probably this length < 16, ys length < 8
      let ret = this.chunk.slice(0);
      outer: for (let i = 0; i < ys.chunk.length; i = i + 2) {
        if (ys.chunk[i + 1] === v) {
          continue;
        }
        for (let k = 0; k < ret.length; k = k + 2) {
          if (DATA_EQUAL(ys.chunk[i], ret[k])) {
            ret[k + 1] = ys.chunk[i + 1];
            continue outer;
          }
        }
        ret.push(ys.chunk[i], ys.chunk[i + 1]);
      }
      return new CalcitSliceMap(ret);
    }

    return this.turnMap().mergeSkip(ys, v);
  }

  /** TODO implement diff with low level code, opens opportunity for future optimizations */
  diffNew(ys: CalcitSliceMap | CalcitMap): CalcitMap {
    return this.turnMap().diffNew(ys);
  }

  /** TODO implement diff with low level code, opens opportunity for future optimizations */
  diffKeys(ys: CalcitMap | CalcitSliceMap): CalcitSet {
    let ret: Array<CalcitValue> = [];
    let ks = this.keysArray();
    for (let i = 0; i < ks.length; i++) {
      let k = ks[i];
      if (!ys.contains(k)) {
        ret.push(k);
      }
    }
    return new CalcitSet(ret);
  }

  /** TODO implement diff with low level code, opens opportunity for future optimizations */
  commonKeys(ys: CalcitMap | CalcitSliceMap): CalcitSet {
    let ret: Array<CalcitValue> = [];
    let ks = this.keysArray();
    for (let i = 0; i < ks.length; i++) {
      let k = ks[i];
      if (ys.contains(k)) {
        ret.push(k);
      }
    }
    return new CalcitSet(ret);
  }
}
