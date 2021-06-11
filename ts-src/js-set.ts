import { CrDataValue } from "./js-primes";
import { toString } from "./calcit-data";

export let cloneSet = (xs: Set<CrDataValue>): Set<CrDataValue> => {
  if (!(xs instanceof Set)) {
    throw new Error("Expected a set");
  }
  var result: Set<CrDataValue> = new Set();
  for (let v of xs) {
    result.add(v);
  }
  return result;
};

export class CrDataSet {
  value: Set<CrDataValue>;
  constructor(value: Set<CrDataValue>) {
    this.value = value;
  }
  len() {
    return this.value.size;
  }
  contains(y: CrDataValue) {
    return this.value.has(y);
  }
  include(y: CrDataValue): CrDataSet {
    var result = cloneSet(this.value);
    result.add(y);
    return new CrDataSet(result);
  }
  exclude(y: CrDataValue): CrDataSet {
    var result = cloneSet(this.value);
    result.delete(y);
    return new CrDataSet(result);
  }

  difference(ys: CrDataSet): CrDataSet {
    var result = cloneSet(this.value);
    ys.value.forEach((y) => {
      if (result.has(y)) {
        result.delete(y);
      }
    });
    return new CrDataSet(result);
  }
  union(ys: CrDataSet): CrDataSet {
    var result = cloneSet(this.value);
    ys.value.forEach((y) => {
      if (!result.has(y)) {
        result.add(y);
      }
    });
    return new CrDataSet(result);
  }
  intersection(ys: CrDataSet): CrDataSet {
    let xs = this.value;
    var result: Set<CrDataValue> = new Set();
    ys.value.forEach((y) => {
      if (xs.has(y)) {
        result.add(y);
      }
    });
    return new CrDataSet(result);
  }

  first(): CrDataValue {
    // rather suspicious solution since set has no logic order
    if (this.value.size === 0) {
      return null;
    }
    for (let x of this.value) {
      return x;
    }
  }
  rest(): CrDataSet {
    if (this.value.size == 0) {
      return null;
    }
    let x0 = this.first();
    let ys = cloneSet(this.value);
    ys.delete(x0);
    return new CrDataSet(ys);
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
