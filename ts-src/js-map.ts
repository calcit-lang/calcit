import * as ternaryTree from "@calcit/ternary-tree";

import { CrDataValue } from "./js-primes";

import { TernaryTreeMap, initTernaryTreeMap, mapLen, assocMap, dissocMap, isMapEmpty, Hash, toPairsArray, mapGetDefault } from "@calcit/ternary-tree";

import { isNestedCrData, tipNestedCrData, toString } from "./calcit-data";

/** need to compare by Calcit */
let DATA_EQUAL = (x: CrDataValue, y: CrDataValue): boolean => {
  return x == y;
};

export let overwriteDataComparator = (f: typeof DATA_EQUAL): void => {
  DATA_EQUAL = f;
};

export class CrDataMap {
  cachedHash: Hash;
  /** in arrayMode, only flatten values, not tree structure */
  arrayMode: boolean;
  arrayValue: CrDataValue[];
  value: TernaryTreeMap<CrDataValue, CrDataValue>;
  skipValue: CrDataValue;
  constructor(value: CrDataValue[] | TernaryTreeMap<CrDataValue, CrDataValue>) {
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
