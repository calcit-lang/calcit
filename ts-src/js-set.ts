import { CalcitValue } from "./js-primes";
import { toString } from "./calcit-data";
import { TernaryTreeMap, initTernaryTreeMap, mapLen, assocMap, dissocMap, isMapEmpty, Hash, toPairsArray, mapGetDefault, contains } from "@calcit/ternary-tree";
import * as ternaryTree from "@calcit/ternary-tree";

/** need to compare by Calcit */
let DATA_EQUAL = (x: CalcitValue, y: CalcitValue): boolean => {
  return x === y;
};

export let overwriteSetComparator = (f: typeof DATA_EQUAL): void => {
  DATA_EQUAL = f;
};

export class CalcitSet {
  value: TernaryTreeMap<CalcitValue, boolean>;
  cachedHash: Hash;
  constructor(value: TernaryTreeMap<CalcitValue, boolean> | Array<CalcitValue>) {
    this.cachedHash = null;
    if (Array.isArray(value)) {
      let pairs: [CalcitValue, boolean][] = [];
      outer: for (let idx = 0; idx < value.length; idx++) {
        for (let j = 0; j < pairs.length; j++) {
          if (DATA_EQUAL(pairs[j][0], value[idx])) {
            // skip existed elements
            continue outer;
          }
        }
        pairs.push([value[idx], true]);
      }
      this.value = initTernaryTreeMap(pairs);
    } else {
      this.value = value;
    }
  }
  len() {
    return mapLen(this.value);
  }
  contains(y: CalcitValue) {
    return contains(this.value, y);
  }
  include(y: CalcitValue): CalcitSet {
    var result = this.value;
    result = assocMap(result, y, true);
    return new CalcitSet(result);
  }
  exclude(y: CalcitValue): CalcitSet {
    var result = this.value;
    result = dissocMap(result, y);
    return new CalcitSet(result);
  }

  difference(ys: CalcitSet): CalcitSet {
    let result = this.value;
    for (let k of ternaryTree.toKeys(ys.value)) {
      result = dissocMap(result, k);
    }
    return new CalcitSet(result);
  }
  union(ys: CalcitSet): CalcitSet {
    let result = this.value;
    for (let k of ternaryTree.toKeys(ys.value)) {
      result = assocMap(result, k, true);
    }
    return new CalcitSet(result);
  }
  intersection(ys: CalcitSet): CalcitSet {
    let result: TernaryTreeMap<CalcitValue, boolean> = initTernaryTreeMap([]);
    for (let k of ternaryTree.toKeys(this.value)) {
      if (ys.contains(k)) {
        result = assocMap(result, k, true);
      }
    }
    return new CalcitSet(result);
  }

  first(): CalcitValue {
    // rather suspicious solution since set has no logical order

    if (mapLen(this.value) == 0) {
      return null;
    }

    return toPairsArray(this.value)[0][0];
  }
  rest(): CalcitSet {
    if (mapLen(this.value) == 0) {
      return null;
    }
    let x0 = this.first();
    let result = dissocMap(this.value, x0);
    return new CalcitSet(result);
  }

  toString() {
    let itemsCode = "";
    for (let k of ternaryTree.toKeys(this.value)) {
      itemsCode = `${itemsCode} ${toString(k, true)}`;
    }
    return `(#{}${itemsCode})`;
  }

  values() {
    return [...ternaryTree.toKeys(this.value)];
  }
}
