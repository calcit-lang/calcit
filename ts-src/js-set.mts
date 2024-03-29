import { CalcitValue, isLiteral } from "./js-primes.mjs";
import { toString } from "./calcit-data.mjs";
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
  contains,
  initTernaryTreeMapFromArray,
  initEmptyTernaryTreeMap,
} from "@calcit/ternary-tree";
import * as ternaryTree from "@calcit/ternary-tree";
import { CalcitSliceList } from "./js-list.mjs";

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
      this.value = initTernaryTreeMapFromArray(pairs);
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
    let result: TernaryTreeMap<CalcitValue, boolean> = initEmptyTernaryTreeMap();
    for (let k of ternaryTree.toKeys(this.value)) {
      if (ys.contains(k)) {
        result = assocMap(result, k, true);
      }
    }
    return new CalcitSet(result);
  }

  destruct(): CalcitSliceList {
    if (mapLen(this.value) === 0) {
      return null;
    }
    // rather suspicious solution since set has no logical order
    let x0 = toPairsArray(this.value)[0][0];

    let result = dissocMap(this.value, x0);
    return new CalcitSliceList([x0, new CalcitSet(result)]);
  }

  toString(disableJsDataWarning: boolean = false) {
    let itemsCode = "";
    for (let k of ternaryTree.toKeys(this.value)) {
      itemsCode = `${itemsCode} ${toString(k, true, disableJsDataWarning)}`;
    }
    return `(#{}${itemsCode})`;
  }

  values() {
    return [...ternaryTree.toKeys(this.value)];
  }

  nestedDataInChildren(): boolean {
    for (let k of ternaryTree.toKeys(this.value)) {
      if (!isLiteral(k)) {
        return true;
      }
    }
    return false;
  }
}
