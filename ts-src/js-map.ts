import * as ternaryTree from "@calcit/ternary-tree";

import { CalcitValue } from "./js-primes";
import { CalcitSet } from "./js-set";

import { TernaryTreeMap, initTernaryTreeMap, mapLen, assocMap, dissocMap, isMapEmpty, Hash, toPairsArray, mapGetDefault } from "@calcit/ternary-tree";

import { isNestedCalcitData, tipNestedCalcitData, toString } from "./calcit-data";

/** need to compare by Calcit */
let DATA_EQUAL = (x: CalcitValue, y: CalcitValue): boolean => {
  return x === y;
};

export let overwriteDataComparator = (f: typeof DATA_EQUAL): void => {
  DATA_EQUAL = f;
};

// a reference that equals to no other value(mainly for telling from `null`)
let fakeUniqueSymbol = [] as any;

export class CalcitMap {
  cachedHash: Hash;
  /** in arrayMode, only flatten values, not tree structure */
  arrayMode: boolean;
  arrayValue: CalcitValue[];
  value: TernaryTreeMap<CalcitValue, CalcitValue>;
  skipValue: CalcitValue;
  constructor(value: CalcitValue[] | TernaryTreeMap<CalcitValue, CalcitValue>) {
    if (value == null) {
      this.arrayMode = true;
      this.arrayValue = [];
    } else if (Array.isArray(value)) {
      this.arrayMode = true;
      this.arrayValue = value;
    } else {
      this.arrayMode = false;
      this.value = value;
    }
  }
  turnMap() {
    if (this.arrayMode) {
      var dict: Array<[CalcitValue, CalcitValue]> = [];
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
  get(k: CalcitValue) {
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
  assoc(k: CalcitValue, v: CalcitValue) {
    if (this.arrayMode && this.arrayValue.length <= 16) {
      let ret = this.arrayValue.slice(0);
      for (let i = 0; i < ret.length; i += 2) {
        if (DATA_EQUAL(k, ret[i])) {
          ret[i + 1] = v;
          return new CalcitMap(ret);
        }
      }
      ret.push(k, v);
      return new CalcitMap(ret);
    } else {
      this.turnMap();
      return new CalcitMap(assocMap(this.value, k, v));
    }
  }
  dissoc(k: CalcitValue) {
    if (this.arrayMode && this.arrayValue.length <= 16) {
      let ret: CalcitValue[] = [];
      for (let i = 0; i < this.arrayValue.length; i += 2) {
        if (!DATA_EQUAL(k, this.arrayValue[i])) {
          ret.push(this.arrayValue[i], this.arrayValue[i + 1]);
        }
      }
      return new CalcitMap(ret);
    } else {
      this.turnMap();
      return new CalcitMap(dissocMap(this.value, k));
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
    if (this.arrayMode) {
      return this.arrayValue.length === 0;
    } else {
      return isMapEmpty(this.value);
    }
  }
  pairs(): Array<[CalcitValue, CalcitValue]> {
    if (this.arrayMode) {
      let ret: Array<[CalcitValue, CalcitValue]> = [];
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
  keysArray(): Array<CalcitValue> {
    if (this.arrayMode) {
      let ret: Array<CalcitValue> = [];
      let size = this.arrayValue.length >> 1;
      for (let i = 0; i < size; i++) {
        let pos = i << 1;
        ret.push(this.arrayValue[pos]);
      }
      return ret;
    } else {
      return [...ternaryTree.toKeys(this.value)];
    }
  }
  contains(k: CalcitValue) {
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
  merge(ys: CalcitMap) {
    return this.mergeSkip(ys, fakeUniqueSymbol);
  }
  mergeSkip(ys: CalcitMap, v: CalcitValue) {
    if (ys == null) {
      return this;
    }

    if (!(ys instanceof CalcitMap)) {
      console.error("value:", v);
      throw new Error("Expected map to merge");
    }

    if (this.arrayMode && ys.arrayMode && this.arrayValue.length + ys.arrayValue.length <= 24) {
      // probably this length < 16, ys length < 8
      let ret = this.arrayValue.slice(0);
      outer: for (let i = 0; i < ys.arrayValue.length; i = i + 2) {
        if (ys.arrayValue[i + 1] === v) {
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
      return new CalcitMap(ret);
    }

    this.turnMap();

    if (ys.arrayMode) {
      let ret = this.value;
      let size = ys.arrayValue.length >> 1;
      for (let i = 0; i < size; i++) {
        let pos = i << 1;
        if (ys.arrayValue[pos + 1] === v) {
          continue;
        }
        ret = assocMap(ret, ys.arrayValue[pos], ys.arrayValue[pos + 1]);
      }
      return new CalcitMap(ret);
    } else {
      return new CalcitMap(ternaryTree.mergeSkip(this.value, ys.value, v));
    }
  }

  /** TODO implement diff with low level code, opens opportunity for future optimizations */
  diffNew(ys: CalcitMap): CalcitMap {
    this.turnMap();
    let zs = this.value;
    if (ys.arrayMode) {
      let size = ys.arrayValue.length >> 1;
      for (let i = 0; i < size; i++) {
        let pos = i << 1;
        let k = ys.arrayValue[pos];
        if (ternaryTree.contains(zs, k)) {
          zs = ternaryTree.dissocMap(zs, k);
        }
      }
      return new CalcitMap(zs);
    } else {
      let ysKeys = ys.keysArray();
      for (let i = 0; i < ysKeys.length; i++) {
        let k = ysKeys[i];
        if (ternaryTree.contains(zs, k)) {
          zs = ternaryTree.dissocMap(zs, k);
        }
      }
      return new CalcitMap(zs);
    }
  }

  /** TODO implement diff with low level code, opens opportunity for future optimizations */
  diffKeys(ys: CalcitMap): CalcitSet {
    let ret: Set<CalcitValue> = new Set();
    let ks = this.keysArray();
    for (let i = 0; i < ks.length; i++) {
      let k = ks[i];
      if (!ys.contains(k)) {
        ret.add(k);
      }
    }
    return new CalcitSet(ret);
  }

  /** TODO implement diff with low level code, opens opportunity for future optimizations */
  commonKeys(ys: CalcitMap): CalcitSet {
    let ret: Set<CalcitValue> = new Set();
    let ks = this.keysArray();
    for (let i = 0; i < ks.length; i++) {
      let k = ks[i];
      if (ys.contains(k)) {
        ret.add(k);
      }
    }
    return new CalcitSet(ret);
  }
}
