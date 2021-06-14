import { CalcitValue } from "./js-primes";
import { toString } from "./calcit-data";

import { Hash } from "@calcit/ternary-tree";

export let cloneSet = (xs: Set<CalcitValue>): Set<CalcitValue> => {
  if (!(xs instanceof Set)) {
    throw new Error("Expected a set");
  }
  var result: Set<CalcitValue> = new Set();
  for (let v of xs) {
    result.add(v);
  }
  return result;
};

export class CalcitSet {
  value: Set<CalcitValue>;
  cachedHash: Hash;
  constructor(value: Set<CalcitValue>) {
    this.cachedHash = null;
    this.value = value;
  }
  len() {
    return this.value.size;
  }
  contains(y: CalcitValue) {
    return this.value.has(y);
  }
  include(y: CalcitValue): CalcitSet {
    var result = cloneSet(this.value);
    result.add(y);
    return new CalcitSet(result);
  }
  exclude(y: CalcitValue): CalcitSet {
    var result = cloneSet(this.value);
    result.delete(y);
    return new CalcitSet(result);
  }

  difference(ys: CalcitSet): CalcitSet {
    var result = cloneSet(this.value);
    ys.value.forEach((y) => {
      if (result.has(y)) {
        result.delete(y);
      }
    });
    return new CalcitSet(result);
  }
  union(ys: CalcitSet): CalcitSet {
    var result = cloneSet(this.value);
    ys.value.forEach((y) => {
      if (!result.has(y)) {
        result.add(y);
      }
    });
    return new CalcitSet(result);
  }
  intersection(ys: CalcitSet): CalcitSet {
    let xs = this.value;
    var result: Set<CalcitValue> = new Set();
    ys.value.forEach((y) => {
      if (xs.has(y)) {
        result.add(y);
      }
    });
    return new CalcitSet(result);
  }

  first(): CalcitValue {
    // rather suspicious solution since set has no logic order
    if (this.value.size === 0) {
      return null;
    }
    for (let x of this.value) {
      return x;
    }
  }
  rest(): CalcitSet {
    if (this.value.size == 0) {
      return null;
    }
    let x0 = this.first();
    let ys = cloneSet(this.value);
    ys.delete(x0);
    return new CalcitSet(ys);
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
